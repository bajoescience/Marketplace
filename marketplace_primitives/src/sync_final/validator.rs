use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Owner, voting::{self, Role, VotingProof}};

use crate::{BlockHeader, Verify, WRVote};

// This is a validator role
const ROLE: Role = Role::Validator;

/// Identity of a block validator, and their proof of participation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Validator {
    proof: VotingProof,
}

impl Validator {
    /// Build a new validator using a block id
    pub fn new(owner: &Owner, seed: &[u8]) -> Self {
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
impl Validator {
    /// Fetch public key
    pub fn pk(&self) -> &[u8] {
        self.proof.pk()
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

    // Validated blockheader
    header: BlockHeader,

    // Store subuser count
    // to be used as a vote count in
    // whiteroom
    subusers: u64,
}

impl Validated {
    /// Create a new Validated using the block id and validator
    pub fn new(header: BlockHeader, validator: Validator, subusers: u64) -> Self {
        Self {
            header,
            voter: validator,
            subusers
        }
    }
}

// Getter methods
impl Validated {
    pub fn get_vrf_opt(&self) -> AgentResult<ID> {
        self.voter.verify(&self.header.id())
    }
}

// This method does not verify the subusers,
// it only validates the structure of the validated
impl Verify for Validated {
    fn verify(&self) -> AgentResult<()> {
        // Verify validator using block hash
        self.voter.verify(&self.header.id())?;

        Ok(())
    }
}

impl PartialEq for Validated {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl WRVote for Validated {
    fn as_vote(&self) -> ID {
        self.header.id()
    }

    fn votes(&self) -> usize {
        self.subusers as usize
    }
}