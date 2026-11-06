use std::{collections::HashMap, format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock, Owner, voting::{self, Role, VotingProof}};

use crate::{Verify, sync_final::{Block, voter::Voter}};

// This is a validator role
const ROLE: Role = Role::Proposer;

/// This structure is used to prove a node's status as proposer.
/// it is added to the `BlockHeader`
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposer {
    // Prove right to vote
    proof: VotingProof,
    block: Block,
}

impl Proposer {
    /// Build a new proposal using the previous block id
    pub fn new(owner: &Owner, block: Block) -> Self {
        // The seed is gotten from the previous block hash
        let seed = block.header().prev();

        let votingproof = voting::vrf(owner, ROLE, &seed);

        Self {
            proof: votingproof,
            block
        }
    }

    /// Verify a proposer identity and return it's VRF output
    /// which can be used to calculate subusers.
    pub fn verify(&self, seed: &[u8]) -> AgentResult<ID> {
        voting::prove_vrf(&self.proof, seed, ROLE)
    }
}

// Getter methods
impl Proposer {
    /// Fetch public key
    pub fn pk(&self) -> &[u8] {
        self.proof.pk()
    }

    /// Get block
    pub fn block(&self) -> &Block {
        &self.block
    }
}

impl Voter for Proposer {
    /// Get lock object representing proposer
    fn owner(&self) -> Lock {
        Lock::SIG(
            functions::hash(self.pk())
        )
    }

    fn get_vrf_opt(&self) -> AgentResult<ID> {
        let seed = self.block.header().prev();

        self.verify(&seed)
    }
}

// Verify proposer used block id as seed, and that the proposer is valid
impl Verify for Proposer {
    fn verify(&self) -> AgentResult<()> {
        let seed = self.block.header().prev();

        self.verify(&seed)?;
        Ok(())
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposal {
    voter: Proposer,

    // Authenticate proposer
    auth: HashMap<ID, Key>
}

impl Proposal {
    pub fn new(proposer: Proposer, auth: HashMap<ID, Key>) -> Self {
        Self {
            voter: proposer,
            auth
        }
    }
}

// Getter methods
impl Proposal {   
    pub fn proposer(&self) -> &Proposer {
        &self.voter
    }

    pub fn block(&self) -> &Block {
        self.proposer().block()
    }
}

// Verify proposal was signed by proposer owner
impl Verify for Proposal {
    fn verify(&self) -> AgentResult<()> {
        let owner = self.proposer().owner();

        // Get key from auth map
        let Some(key) = self.auth.get(&owner.id()) else {
            return Err(format!(
                "Err: Key for Lock {} does not exist",
                functions::from_bytes(&owner.id())
            ))
        };

        owner.unlock(key, &self.proposer().id())
    }
}

#[cfg(test)]
mod tests {
    use marketplace_wallet::Owner;

    use crate::{
        Verify,
        sync_final::{Block, Proposer, Proposal, voter::Voter},
    };

    // A proposer should retain its block and create a verifiable proof from its previous-block seed.
    #[test]
    fn proposer_uses_previous_block_as_vrf_seed() {
        let owner = Owner::new_sig();
        let block = Block::genesis();
        let expected_id = block.id();
        let seed = block.header().prev();
        let proposer = Proposer::new(&owner, block);

        assert_eq!(proposer.block().id(), expected_id);
        assert_eq!(proposer.pk(), owner.pk());
        assert!(proposer.verify(&seed).is_ok());
        assert!(proposer.get_vrf_opt().is_ok());
        assert!(Verify::verify(&proposer).is_ok());
    }

    // A proposal should expose the block contained by its proposer.
    #[test]
    fn proposal_exposes_proposed_block() {
        let owner = Owner::new_sig();
        let block = Block::genesis();
        let expected_id = block.id();
        let proposer = Proposer::new(&owner, block);
        let proposal = Proposal::new(proposer, Default::default());

        assert_eq!(proposal.block().id(), expected_id);
        assert_eq!(proposal.proposer().block().id(), expected_id);
    }
}
