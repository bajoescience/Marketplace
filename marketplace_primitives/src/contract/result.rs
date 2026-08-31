use std::{collections::{HashMap, hash_map::Entry}, format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, WorkSize}};
use marketplace_wallet::{Key, Lock, crypto::WRProof};
use crate::{Verify, WRVote, helpers::objects::{ID, IdHash, WorkAddr}};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ResultInfo {
    // Result hash of work executed
    opt_hash: ID,

    // WU spent
    spent: WorkSize,
}

impl ResultInfo {
    // New result
    pub fn new(opt_hash: ID, spent: WorkSize) -> Self {
        Self {
            opt_hash,
            spent
        }
    }

    pub fn opt_hash(&self) -> ID {
        self.opt_hash
    }

    pub fn spent(&self) -> WorkSize {
        self.spent
    }
}

impl PartialEq for ResultInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

/// The ResultPtr acts like a smart pointer, and is used by whiteroom members
/// to reference the following:
/// 
/// A result of the work execution.
/// 
/// The Work executed by it's WorkPtr instance.
/// 
/// The ResultPtr also contains metadata which includes
/// the following:
/// 
/// Whiteroom transaction: This is a transaction that a whiteroom memeber creates
/// by spending the whiteroom output from the transaction in the workPtr

// Pointer to work output
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ResultPtr {
    // ID of referenced WorkPtr
    work_id: ID,

    // Address of where work result is stored
    res_addr: WorkAddr,

    // Result information
    result: ResultInfo,

    // Whiteroom Proof of Inclusion
    wr_proof: WRProof,

    // Whiteroom memeber
    witness: Lock,

    // Prove ownership of resultptr
    #[borsh(skip)]
    auth: HashMap<ID, Key>
}

impl ResultPtr {
    // For two ResultPtr instances to be considered the compatible,
    // They must have the same result hash
    // This is to indicate they have the same result
    pub fn is_same(&self, other: &Self) -> bool {
        if self.result == other.result {
            return true
        }

        false
    }
}

impl PartialEq for ResultPtr {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl ResultPtr {
    pub fn new(
        work_id: ID,
        result: ResultInfo, 
        wr_proof: WRProof,
        res_addr: WorkAddr, 
        witness: Lock,
    ) -> Self {
        Self {
            work_id,
            res_addr,
            witness,
            result,
            wr_proof,
            auth: HashMap::new(),
        }
    }
}

// Getter methods
impl ResultPtr {
    pub fn work_id(&self) -> ID {
        self.work_id
    }

    pub fn result(&self) -> &ResultInfo {
        &self.result
    } 

    pub fn wr_proof(&self) -> &WRProof {
        &self.wr_proof
    }

    pub fn res_addr(&self) -> &WorkAddr {
        &self.res_addr
    }

    pub fn witness(&self) -> Lock {
        self.witness
    }
}

// Setter methods
impl ResultPtr {
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
}

/// The ResultPtr proof is verified later
/// but as long as the whiteroom member is known
/// and used to sign the message, we can use that
/// member id later in the proof. 
impl Verify for ResultPtr {
    /// NOTE: Whiteroom proof is verified by contract
    /// So this method does not fully validate the ResultPtr instance
    /// as the WorkPtr and BlockHeader is needed
    /// 
    /// To fully validate it, use a Contract instance with the 
    /// method Contract.validate_result(ResultPtr)
    fn verify(&self) -> AgentResult<()> {
        // Unlock whiteroom
        let witness = self.witness();

        // Ensure whiteroom owns the proof
        let Lock::WHITEROOM(pk_hash) = witness else {
            return Err(format!(
                "Error: Whiteroom Tx input must locked by Whiteroom",
            ))
        };

        if pk_hash != functions::hash(self.wr_proof().pk_bytes()) {
            return Err(format!(
                "Error: Whiteroom tx owner not the same as proof owner"
            ))
        }

        // Unlock Result Ptr to prove full ownership
        if let Some(key) = self.auth.get(&witness.id()) {
            witness.unlock(key, &self.id())?;
        } else {
            return Err(format!(
                "Error: Key does not exist for workptr {}",
                functions::from_bytes(&witness.id()), 
            ));
        };

        Ok(())
    }
}

// Each ResultPtr can be represented as a Whiteroom Vote
impl WRVote for ResultPtr {
    fn as_vote(&self) -> ID {
        self.result.id()
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_eq};

use marketplace_helpers::{functions::dum_bytes, objects::WU};
use marketplace_wallet::{Owner, crypto::Crypto};

use super::*;

    pub fn wr_proof(owner: &Owner) -> WRProof {
        let crypto = Crypto::new(owner);
        crypto.attempt_wr(
            &dum_bytes(), 
            functions::vdf_difficulty(
                WU::try_from(300000).unwrap()
            )
        ).unwrap()
    }

    pub fn res_ptr(spent: WorkSize, owner: &Owner, wr_proof: WRProof) -> ResultPtr {
        let result = ResultInfo {
            opt_hash: dum_bytes(),
            spent,
        };

        ResultPtr::new(
            dum_bytes(),
            result,
            wr_proof,
            dum_bytes(),  
            owner.as_wr_lock(),
        )
    }

    // correct usage of Resptr API
    #[test]
    fn test_res_ptr() -> AgentResult<()> {
        // Owner
        let owner = Owner::new_sig();

        let spent = WU::try_from(20000).unwrap();

        // Prove ownership of two whiteroom inputs using VDF + VRF
        // and build res_ptr
        let wr_proof = wr_proof(&owner);
        let mut resptr = res_ptr(
            WorkSize::build(spent).unwrap(), 
            &owner, 
            wr_proof
        );

        let key = owner.sign(&resptr.id());
        resptr.add_auth(owner.as_wr_lock(), key, false).unwrap();

        resptr.verify()
    }

    // Resultptr tx with whiteroom input that has a different unlock
    // pub key hash than what is in WRProof should fail
    #[test]
    fn test_res_ptr_wrong_proof() {
        // wr owner
        let wr_owner = Owner::new_sig();

        // false owner
        let f_owner = Owner::new_sig();

        let spent = WU::try_from(20000).unwrap();

        // Prove ownership of two whiteroom inputs using VDF + VRF
        // and build res_ptr but 
        // build WRproof with false owner
        let wr_proof = wr_proof(&f_owner);
        let mut resptr = res_ptr(
            WorkSize::build(spent).unwrap(), 
            &wr_owner, 
            wr_proof
        );

        // Sign with real WR owner
        let key = wr_owner.sign(&resptr.id());
        resptr.add_auth(wr_owner.as_wr_lock(), key, false).unwrap();

        let err = resptr.verify().unwrap_err();

        assert_eq!(err, "Error: Whiteroom tx owner not the same as proof owner");
    }

    // Result ptr tx must be locked by a whiteroom
    // Only whiteroom can be receivers
    #[test]
    pub fn lock_res_ptr_tx() {
        let owner = Owner::new_sig();
        let spent = WU::try_from(20000).unwrap();

        let wr_proof = wr_proof(&owner);
        let mut res_ptr = res_ptr(
            WorkSize::build(spent).unwrap(),
            &owner, 
            wr_proof
        );

        res_ptr.add_auth(
            owner.as_lock(), 
            owner.sign(&res_ptr.id()), 
            false
        ).unwrap();

        let err = res_ptr.verify().unwrap_err();

        assert!(
            err.contains("Error: Key does not exist for workptr"),
        )
    }
}