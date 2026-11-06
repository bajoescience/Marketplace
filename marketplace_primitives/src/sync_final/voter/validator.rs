use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock, Owner, voting::{self, Role, VotingProof}};

use crate::{Verify, par_exec::WRVote, sync_final::{BlockHeader, Proposer, voter::Voter}};

// This is a validator role
const ROLE: Role = Role::Validator;

/// Identity of a block validator, and their proof of participation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Validator {
    proof: VotingProof,

    // Proposer ID
    proposer: ID,

    // Validated blockheader
    header: BlockHeader,
}

impl Validator {
    /// Build a new validator using a block id
    pub fn new(owner: &Owner, seed: &[u8], proposer: &Proposer) -> Self {
        let votingproof = voting::vrf(owner, ROLE, seed);

        let header = proposer.block().header();

        Self {
            proof: votingproof,
            header: header.clone(),
            proposer: proposer.id(),
        }
    }

    /// Verify a proposer identity and return it's VRF output
    /// which can be used to calculate subusers.
    pub fn verify(&self, seed: &[u8]) -> AgentResult<ID> {
        voting::prove_vrf(&self.proof, seed, ROLE)
    }
}

// Getter methods
impl Validator {
    /// Fetch public key
    pub fn pk(&self) -> &[u8] {
        self.proof.pk()
    }
}

impl Voter for Validator {
    /// Get lock object representing proposer
    fn owner(&self) -> Lock {
        Lock::SIG(
            functions::hash(self.pk())
        )
    }

    fn get_vrf_opt(&self) -> AgentResult<ID> {
        let seed = self.header.prev();

        self.verify(&seed)
    }
}


/// The `Validated` is the structure sent to the network to validate a 
/// proposed `BlockHeader`
/// 
/// The `Validated` struct is not stored, but is only used to validate that a block is 
/// the appropiate next block in the chain.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Validated {
    // Validator
    voter: Validator,

    // Store subuser count
    // to be used as a vote count in
    // whiteroom
    subusers: u64,

    // Authenticate Validated
    auth: HashMap<ID, Key>
}

impl Validated {
    /// Create a new Validated using the block id and validator
    pub fn new(validator: Validator, subusers: u64, auth: HashMap<ID, Key>) -> Self {
        Self {
            voter: validator,
            subusers,
            auth
        }
    }
}

// Getter methods
impl Validated {
    pub fn validator(&self) -> &Validator {
        &self.voter
    }
}

// This method does not verify the subusers,
// it only validates the structure of the validated
impl Verify for Validated {
    fn verify(&self) -> AgentResult<()> {
        let owner = self.voter.owner();

        let Some(key) = self.auth.get(&owner.id()) else {
            return Err(format!(
                "Err: Key for Lock {} does not exist",
                functions::from_bytes(&owner.id())
            ))
        };

        owner.unlock(key, &self.validator().id())
    }
}

impl PartialEq for Validated {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl WRVote for Validated {
    fn as_vote(&self) -> ID {
        self.voter.header.id()
    }

    fn votes(&self) -> usize {
        self.subusers as usize
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use marketplace_helpers::{functions, objects::IdHash};
    use marketplace_wallet::Owner;

    use crate::{
        Verify,
        par_exec::WRVote,
        sync_final::{Block, Proposer, Validated, Validator, voter::Voter},
    };

    // A validator should retain the voter's public key and verify its proposer-seeded proof.
    #[test]
    fn validator_proves_voter_identity() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());
        let seed = proposer.block().header().prev();
        let validator = Validator::new(&owner, &seed, &proposer);

        assert_eq!(validator.pk(), owner.pk());
        assert!(validator.verify(&seed).is_ok());
        assert!(validator.get_vrf_opt().is_ok());
        assert_eq!(
            validator.owner(),
            marketplace_wallet::Lock::SIG(functions::hash(&owner.pk()))
        );
    }

    // A validated vote should expose the validator and its declared vote weight.
    #[test]
    fn validated_vote_exposes_validator_and_weight() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());
        let seed = proposer.block().header().prev();
        let validator = Validator::new(&owner, &seed, &proposer);
        let vote_id = proposer.block().header().id();
        let validated = Validated::new(validator, 4, HashMap::new());

        assert_eq!(validated.validator().pk(), owner.pk());
        assert_eq!(validated.as_vote(), vote_id);
        assert_eq!(validated.votes(), 4);
    }

    // A validated vote should authenticate using a key associated with its validator lock.
    #[test]
    fn validated_vote_verifies_with_validator_authentication() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());
        let seed = proposer.block().header().prev();
        let validator = Validator::new(&owner, &seed, &proposer);
        let validator_id = validator.id();
        let validator_lock = validator.owner();
        let key = owner.sign(&validator_id);
        let mut auth = HashMap::new();
        auth.insert(validator_lock.id(), key);
        let validated = Validated::new(validator, 1, auth);

        assert!(validated.verify().is_ok());
    }

    // A validated vote without its validator key should fail structural verification.
    #[test]
    fn validated_vote_requires_validator_authentication() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());
        let seed = proposer.block().header().prev();
        let validator = Validator::new(&owner, &seed, &proposer);
        let validated = Validated::new(validator, 1, HashMap::new());

        assert!(validated.verify().is_err());
    }
}