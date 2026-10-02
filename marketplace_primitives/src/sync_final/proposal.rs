use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{objects::{AgentResult, ID}};
use marketplace_wallet::{Owner, voting::{self, Role, VotingProof}};

use crate::{Block, Verify};

// This is a validator role
const ROLE: Role = Role::Proposer;

/// This structure is used to prove a node's status as proposer.
/// it is added to the `BlockHeader`
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposer {
    // Prove right to vote
    proof: VotingProof,
}

impl Proposer {
    /// Build a new proposal using the previous block id
    pub fn build(owner: &Owner, seed: &[u8]) -> Self {
        let votingproof = voting::vrf(owner, ROLE, seed);

        Self {
            proof: votingproof,
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
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposal {
    pub block: Block,
    pub voter: Proposer
}

impl Proposal {
    pub fn new(owner: &Owner, block: Block) -> Self {
        let proposer = Proposer::build(owner, &block.id());

        Self {
            block,
            voter: proposer
        }
    }
}

// Getter methods
impl Proposal {
    pub fn get_vrf_opt(&self) -> AgentResult<ID> {
        self.voter.verify(&self.block.id())
    }
}

// Verify proposer used block id as seed, and that the proposer is valid
impl Verify for Proposal {
    fn verify(&self) -> AgentResult<()> {
        self.voter.verify(&self.block.id())?;

        Ok(())
    }
}

