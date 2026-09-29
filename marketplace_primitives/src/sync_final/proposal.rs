use std::{collections::{HashMap, hash_map::Entry}, format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock, Owner, voting::{self, Role, VotingProof}};

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
    /// Build a new proposal using a block id
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


/// # Proposal
/// 
/// This is the structure sent by a block proposer to put forward a block to the network.
/// a node uses it's VRF to vote
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposal {
    // proposer
    voter: Lock,

    // Proposed block
    block: Block,

    // Prove ownership of proposal
    #[borsh(skip)]
    auth: HashMap<ID, Key>,
}

impl Proposal {
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

    pub fn new(owner: &Owner, proposer: Proposer, mut block: Block) -> AgentResult<Self> {
        block.add_proposer(proposer);

        let voter = owner.as_lock();

        let mut proposal = Self {
            block,
            voter,
            auth: HashMap::new(),
        };

        // Sign block
        let key = owner.sign(&proposal.id());

        proposal.add_auth(voter, key, false)?;

        Ok(proposal)
    }
}

// This method does not verify the Voting proposal itself,
// it only validates the structure of the proposal
impl Verify for Proposal {
    fn verify(&self) -> AgentResult<()> {
        // Ensure proposer exists in block
        let Some(proposer) = self.block.header().proposer() else {
            return Err(format!(
                "Error: Proposer not found in block"
            ))
        };

        // Ensure that Lock and proof public key match
        let Lock::SIG(pk_hash) = self.voter else {
            return Err(format!(
                "Error: Lock object is not valid"
            ));
        };

        if pk_hash != functions::hash(proposer.pk()) {
            return Err(format!(
                "Error: public key not valid"
            ))
        }

        // Unlock lock to prove ownership
        if let Some(key) = self.auth.get(&self.voter.id()) {
            self.voter.unlock(key, &self.id())?;
        } else {
            return Err(format!(
                "Error: Key does not exist for proposer {}",
                functions::from_bytes(&proposer.id()), 
            ));
        };

        Ok(())
    }
}