use std::{format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult}};
use marketplace_wallet::{Lock, Owner, voting::{self, Role, VotingProof}};

use crate::Verify;

/// # Proposal
/// 
/// This is the structure sent by a block proposer to put forward a block to the network.
/// a node uses it's VRF to vote
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Proposal {
    // Identity
    voter: Lock,

    // Prove right to vote
    proof: VotingProof,
}

impl Proposal {
    /// Build a new proposal
    pub fn build(owner: &Owner, seed: &[u8]) -> Self {
        let role = Role::Proposer;

        let votingproof = voting::vrf(owner, role, seed);

        Self {
            voter: owner.as_lock(),
            proof: votingproof,
        }
    }
}

// Getter methods
impl Proposal {
    pub fn voter(&self) -> Lock {
        self.voter
    }
}

// This method does not verify the Voting proposal itself,
// it only validates the structure of the proposal
impl Verify for Proposal {
    fn verify(&self) -> AgentResult<()> {
        // Ensure that Lock and proof public key match
        let Lock::SIG(pk_hash) = self.voter else {
            return Err(format!(
                "Error: Lock object is not valid"
            ));
        };

        if pk_hash != functions::hash(self.proof.pk()) {
            return Err(format!(
                "Error: public key not valid"
            ))
        }

        Ok(())
    }
}