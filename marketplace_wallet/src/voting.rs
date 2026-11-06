use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, WU}};
use schnorrkel::{PublicKey, vrf::{VRF_PREOUT_LENGTH, VRF_PROOF_LENGTH, VRFPreOut, VRFProof}};
use statrs::distribution::{Binomial, DiscreteCDF};

pub const EXPECTED_VOTERS: usize = 25;

/// BFT threshold of Expected Voters Size
pub const fn voting_threshold() -> usize {
    let f = (EXPECTED_VOTERS - 1) / 3;

    (2 * f) + 1
}

use crate::Owner;

/// # Role
/// 
/// This is the role status of the entity voting on a block.
/// 
/// The roles consists of:
/// 
/// - Proposer
/// 
/// - Validator
#[derive(Debug, Clone, Copy, BorshSerialize, BorshDeserialize)]
pub enum Role {
    Proposer,
    Validator
}

// VRF module for block voting
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct VotingProof {
    vrf_pre_opt: [u8; VRF_PREOUT_LENGTH],
    vrf_proof: [u8; VRF_PROOF_LENGTH],
    pk_bytes: ID,
}

impl VotingProof {
    pub fn pk(&self) -> &[u8] {
        &self.pk_bytes
    }
}


/// Voting Proof constructor using VRF for block voting 
/// depending on the voting role
/// 
/// The seed is the hash of the previous block
///
/// This function returns the VRF output and proof
pub fn vrf(owner: &Owner, role: Role, seed: &[u8]) -> VotingProof {
    // Input hash of seed and role into vrf construction
    let role_string = match role {
        Role::Proposer => String::from("proposer"),
        Role::Validator => String::from("validator")
    };

    let ctx = schnorrkel::signing_context(role_string.as_bytes());

    let trsc = ctx.bytes(
        &functions::seed(seed, role_string.as_bytes())
    );

    // Generate proof
    let (io, proof, _) = owner.keypair()
        .vrf_sign(trsc);

    VotingProof { 
        vrf_pre_opt: io.to_preout().to_bytes(), 
        vrf_proof: proof.to_bytes(), 
        pk_bytes: owner.pk(),
    }
}

/// Get output of VRF after proving
pub fn prove_vrf(voting_proof: &VotingProof, seed: &[u8], role: Role) -> AgentResult<ID> {
    let Ok(public) = PublicKey::from_bytes(&voting_proof.pk_bytes) else {
        return Err(format!(
            "Error: Public key is not valid"
        ));
    };

    // Get Input of VRF
    let role_string = match role {
        Role::Proposer => String::from("proposer"),
        Role::Validator => String::from("validator")
    };

    let ctx = schnorrkel::signing_context(role_string.as_bytes());

    let trsc = ctx.bytes(
        &functions::seed(seed, role_string.as_bytes())
    );

    // Verify vrf opt
    match public.vrf_verify(
        trsc, 
        &VRFPreOut::from_bytes(&voting_proof.vrf_pre_opt).unwrap(), 
        &VRFProof::from_bytes(&voting_proof.vrf_proof).unwrap()
    ) {
        Ok((vrf_io, _)) => Ok(
            functions::hash(vrf_io.as_output_bytes())
        ),
        Err(_) => Err(format!(
            "Error: Could not verify VRF output"
        ))
    }
}

/// Get subusers according to algorand subusers algorithm
/// 
/// see https://github.com/onplanetnowhere/AlgorandConsensusProtocolMD#cryptographic-sortition
/// 
/// 
/// `c`      = random value in [0, 1) given by VRFhash / 2 ^ hashlength
/// `weight` = user's currency balance / number of subUsers
/// `p`      = probability that each subUser is selected
///
/// In the Algorand model:
///     p = expected_users / total_currency
pub fn subusers_count(vrf_opt: ID, user_amount: WU, total_currency: WU) -> AgentResult<u64> {
    // Constant value in [0, 1) calculated from VRF output
    let c = functions::hashconstant(vrf_opt);

    // In the Algorand sortition model, p = expected_users / total_currency.
    let p = functions::voter_probability(EXPECTED_VOTERS as u128, total_currency);

    // Take the lower 64 bits as weight
    let weight = u64::try_from(user_amount.inner()).unwrap_or(u64::MAX);

    // X ~ Binomial(weight, p)
    let distribution = Binomial::new(p, weight)
        .map_err(|e| format!("invalid binomial parameters: {e}"))?;

    // Find the binomial interval containing c.
    Ok(distribution.inverse_cdf(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The voting threshold should tolerate the configured BFT fault allowance.
    #[test]
    fn vrf_round_trip_verifies_for_proposer_role() {
        let owner = Owner::new_sig();
        let seed = [11u8; 32];

        let proof = vrf(&owner, Role::Proposer, &seed);
        let vrf_hash = prove_vrf(&proof, &seed, Role::Proposer).unwrap();

        assert_ne!(vrf_hash, [0u8; 32]);
    }

    // A VRF proof generated for one role or seed should fail when checked in another context.
    #[test]
    fn vrf_verification_rejects_wrong_seed_or_role() {
        let owner = Owner::new_sig();
        let seed = [22u8; 32];
        let proof = vrf(&owner, Role::Validator, &seed);

        assert!(prove_vrf(&proof, &seed, Role::Proposer).is_err());
        assert!(prove_vrf(&proof, &[99u8; 32], Role::Validator).is_err());
    }

    // A voter with no currency weight should receive no subuser votes.
    #[test]
    fn subusers_count_returns_zero_for_zero_weight() {
        let vrf_opt = [0u8; 32];
        let amount = WU::default();
        let total_currency = WU::try_from(10_000).unwrap();

        let count = subusers_count(vrf_opt, amount, total_currency).unwrap();

        assert_eq!(count, 0);
    }

    // Subuser selection should not exceed the tested expected-voter bound.
    #[test]
    fn subusers_count_stays_within_the_expected_voter_cap() {
        let vrf_opt = [1u8; 32];
        let amount = WU::try_from(1_000).unwrap();
        let total_currency = WU::try_from(100_000).unwrap();

        let count = subusers_count(vrf_opt, amount, total_currency).unwrap();

        assert!(count <= EXPECTED_VOTERS as u64);
    }

    // The configured threshold should match the BFT formula for the expected voter count.
    #[test]
    fn voting_threshold_matches_bft_quorum() {
        assert_eq!(EXPECTED_VOTERS, 25);
        assert_eq!(voting_threshold(), 17);
    }

    // A voting proof should expose the public key belonging to its owner.
    #[test]
    fn voting_proof_contains_owner_public_key() {
        let owner = Owner::new_sig();
        let proof = vrf(&owner, Role::Validator, &[12; 32]);

        assert_eq!(proof.pk(), &owner.pk());
    }
}
