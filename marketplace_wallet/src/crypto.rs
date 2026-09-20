use std::{format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID}};
use vdf_rs::{VDF, VDFParams, WesolowskiVDF, WesolowskiVDFParams};
use crate::{Owner};

use schnorrkel::{ Keypair, PublicKey, vrf::{VRF_PREOUT_LENGTH, VRF_PROOF_LENGTH, VRFPreOut, VRFProof}};

/// # Whiteroom Proof `WRProof`
/// 
/// The `WRProof` contains the complete context needed to prove 
/// Whiteroom membership (see Marketplace Whitepaper)
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct WRProof {
    vdf_opt: Vec<u8>,
    vrf_pre_opt: [u8; VRF_PREOUT_LENGTH],
    vrf_proof: [u8; VRF_PROOF_LENGTH],
    pk_bytes: ID,
}

impl WRProof {
    fn new(
        vdf_opt: Vec<u8>,
        vrf_pre_opt: [u8; VRF_PREOUT_LENGTH],
        vrf_proof: [u8; VRF_PROOF_LENGTH],
        pk_bytes: ID,
    ) -> Self {
        Self {
            vdf_opt,
            vrf_pre_opt,
            vrf_proof,
            pk_bytes
        }
    }

    pub fn vdf_opt(&self) -> &Vec<u8> {
        &self.vdf_opt
    }

    pub fn vrf_pre_opt(&self) -> VRFPreOut {
        VRFPreOut::from_bytes(&self.vrf_pre_opt).unwrap()
    }

    pub fn vrf_proof(&self) -> VRFProof {
        VRFProof::from_bytes(&self.vrf_proof).unwrap()
    }

    pub fn pk_bytes(&self) -> &[u8] {
        &self.pk_bytes
    }
}

/// # Crypto
/// 
/// `Crypto` module initializes the needed cryptographic primitives
/// to execute a whiteroom challenge.
/// 
/// It is initialized for each identity given.
/// 
/// ## Examples
/// 
/// ```
/// use marketplace_wallet::{Crypto, Owner};
/// 
/// // An identity
/// let owner = Owner::new_sig();
/// 
/// let crypto = Crypto::new(&owner);
/// ```
pub struct Crypto<'a> {
    vdf: WesolowskiVDF,

    // Secret key to VRF
    owner: &'a Owner,
}

impl<'a> Crypto<'a> {
    pub fn new(owner: &'a Owner) -> Self {
        Self {
            vdf: WesolowskiVDFParams(2048).new(),
            owner: owner,
        }
    }

    fn keypair(&self) -> &Keypair {
        self.owner.keypair()
    }

    /// ## Solve VDF
    /// 
    /// The `seed` is the hash of the owner public key and the WorkPtr ID
    /// 
    /// The `diff` is the result of vdf_difficulty function
    /// using the parameters `work_pay` and `VRF threshold`.
    fn solve_vdf(&self, seed: &[u8], diff: u64) -> AgentResult<Vec<u8>> {
        match self.vdf.solve(seed, diff) {
            Ok(proof) => Ok(proof),
            Err(_) => Err(format!(
                "Error: Unable to execute VDF"
            ))
        }
    }

    // Verify VDF using self by own node
    fn verify_vdf_self(&self, seed: &[u8], diff: u64, proof: &Vec<u8>) -> AgentResult<()> {
        match self.vdf.verify(seed, diff, proof) {
            Ok(_) => Ok(()),
            Err(_) => Err(format!(
                "Error: Invalid VDF proof"
            ))
        }
    }

    // Verify VDF without any secret parameters
    fn verify_vdf(seed: &[u8], diff: u64, proof: &Vec<u8>) -> AgentResult<()> {
        match WesolowskiVDFParams(2048)
            .new()
            .verify(seed, diff, proof) {
                Ok(_) => Ok(()),
                Err(_) => Err(format!(
                    "Error: Invalid VDF proof"
                ))
        }
    }

    // Create a new VRF output using seed
    fn get_vrf(
        &self, 
        vdf_opt: &[u8]
    ) -> AgentResult<WRProof> {
        // Input vdf_output into vrf construction
        let ctx = schnorrkel::signing_context(b"wr");

        let trsc = ctx.bytes(vdf_opt);

        // generate proof
        let (io, proof, _) = self.keypair()
            .vrf_sign(trsc);

        Ok(WRProof {
            vdf_opt: vdf_opt.to_vec(),
            vrf_pre_opt: io.to_preout().to_bytes(),
            vrf_proof: proof.to_bytes(),
            pk_bytes: self.keypair().public.to_bytes(),
        })
    }

    /// This method is to verify
    /// for a node with it's secret key
    /// if it has been chosen

    // Verify vrf using pubkey bytes
    fn verify_vrf(
        wr_proof: &WRProof,
    ) -> AgentResult<ID> {
        let Ok(public) = PublicKey::from_bytes(wr_proof.pk_bytes()) else {
            return Err(format!(
                "Error: Public key is not valid"
            ));
        };

        let ctx = schnorrkel::signing_context(b"wr");

        let trsc = ctx.bytes(wr_proof.vdf_opt());

        // Verify vrf opt
        match public.vrf_verify(
            trsc, 
            &wr_proof.vrf_pre_opt(), 
            &wr_proof.vrf_proof()
        ) {
            Ok((vrf_io, _)) => Ok(
                functions::hash(vrf_io.as_output_bytes())
            ),
            Err(_) => Err(format!(
                "Error: Could not verify VRF output"
            ))
        }
    }

    /// This function attempts to join a whiteroom by solving the
    /// `VDF` + `VRF` (Whiteroom challenge) to get a random number which is used to check if
    /// an identity has entered the whiteroom.
    /// 
    /// The `seed` is the hash of the `owner public key` and the `WorkPtr ID`
    /// 
    /// The `diff` is the result of `vdf_difficulty` function
    /// using the parameters `work_pay` and `VRF threshold`.
    pub fn attempt_wr(&self, seed: &ID, diff: u64) -> AgentResult<WRProof> {
        // Solve VDF
        let vdf_opt = self.solve_vdf(seed, diff)?;

        // Use VDF output to initialize VRF
        let wr_proof = self.get_vrf(&vdf_opt)?;

        Ok(wr_proof)
    }

    /// Prove whiteroom membership of an identity using a `WRProof` instance.
    /// 
    /// The `seed` is the hash of the `owner public key` and the `WorkPtr ID`
    /// The `wr_seed` function in the helpers library is useful here.
    /// 
    /// The `diff` is the result of `vdf_difficulty` function
    /// using the parameters `work_pay` and `VRF threshold.
    // Verify whiteroom proof
    pub fn wr_prove(
        wr_proof: &WRProof, 
        seed: &[u8], 
        diff: u64,
    ) -> AgentResult<ID> {
        // Verify VDF
        Self::verify_vdf(seed, diff, wr_proof.vdf_opt())?;

        // Verify VRF
        Self::verify_vrf(wr_proof)
    }
}

#[cfg(test)]
mod tests {
use std::vec;

use marketplace_helpers::{functions::{self, dum_bytes}, objects::WU};

    use super::*;

    // Execute vdf and vrf, and return Whiteroom proof
    #[test]
    fn evaluate_whiteroom() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let crypto = Crypto::new(&owner);

        // Other parameters to verify with.
        let work_id = dum_bytes();
        let vrf_t = [2u8; 32];

        let diff = functions::vdf_difficulty(
            WU::try_from(300000000).unwrap(),
            vrf_t
        );

        // Attempt whiteroom membership
        let wr_proof = crypto.attempt_wr(&work_id, diff)?;

        // Verify whiteroom membership
        let vrf_opt = Crypto::wr_prove(
            &wr_proof, 
            &work_id, 
            diff,
        )?;

        // VRF outputs and threshold
        let vrf_t = [255; 32];

        // assert!(result.is_ok());
        assert!(vrf_opt < vrf_t);
        Ok(())
    }

    // Evaluate vdf using owner
    #[test]
    fn evaluate_vdf() -> AgentResult<()> {
        let owner = Owner::new_sig();

        let crypto = Crypto::new(&owner);

        let work_id = dum_bytes();
        let vrf_t = [2u8; 32];

        let vdf_diff = functions::vdf_difficulty(
            WU::try_from(3000000000).unwrap(),
            vrf_t
        );

        let proof = crypto.solve_vdf(&work_id, vdf_diff)?;

        crypto.verify_vdf_self(&work_id, vdf_diff, &proof)
    }

    // Evaluate vrf
    #[test]
    fn evaluate_vrf() {
        let owner = Owner::new_sig();
        
        let crypto = Crypto::new(&owner);

        let vdf_opt = vec![20];

        let wr_proof = crypto.get_vrf(&vdf_opt).unwrap();

        Crypto::verify_vrf(&wr_proof).unwrap();
    }
}