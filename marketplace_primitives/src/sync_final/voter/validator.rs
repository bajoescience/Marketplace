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