use std::{collections::{HashMap, hash_map::Entry}, format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock, Owner, voting::{self, Role, VotingProof}};

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

    // Prove ownership of Validated
    #[borsh(skip)]
    auth: HashMap<ID, Key>
}

// Getter methods
impl Validated {
    pub fn add_auth(&mut self, lock: Lock, key: Key, force: bool) -> AgentResult<()> {
        match self.auth.entry(lock.id()) {
            Entry::Vacant(entry) => {
                entry.insert(key);
            },
            Entry::Occupied(mut entry) if force => {
                entry.insert(key);
            },
            _ => {
                return Err(format!(
                    "Key already exists for Lock ID {}",
                    functions::from_bytes(&lock.id())
                ))
            }
        };

        Ok(())
    }

    /// Create a new Validated using the block id and validator
    pub fn build(owner: &Owner, validator: Validator, header: BlockHeader) -> AgentResult<Self> {
        let voter = owner.as_lock();

        let mut validated = Self {
            header,
            voter: validator,
            auth: HashMap::new(),
        };

        // Sign block
        let key = owner.sign(&validated.id());

        validated.add_auth(voter, key, false)?;

        Ok(validated)
    }
}

// This method does not verify the subusers,
// it only validates the structure of the validated
impl Verify for Validated {
    fn verify(&self) -> AgentResult<()> {
        // Verify validator using previous block hash
        self.voter.verify(&self.header.prev())?;

        // Create lock from validator public key
        // which can be used to verify validated ownership
        let lock = Lock::SIG(
            functions::hash(self.voter.pk())
        );

        // Unlock lock to prove ownership
        if let Some(key) = self.auth.get(&lock.id()) {
            lock.unlock(key, &self.id())?;
        } else {
            return Err(format!(
                "Error: Key does not exist for validator {}",
                functions::from_bytes(&lock.id()), 
            ));
        };

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
}