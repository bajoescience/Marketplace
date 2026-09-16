use std::{collections::{HashMap, hash_map::Entry}, format};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, WorkSize}};
use marketplace_wallet::{Key, Lock, crypto::WRProof};
use crate::{Verify, WRVote, helpers::objects::{ID, IdHash, WorkAddr}};

/// # Result Info
/// 
/// `Result Info` contains information on a execution result all
/// whiteroom members should agree on.
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

/// # Result Pointer `ResultPtr`
/// 
/// The `ResultPtr` is a pointer like type used by whiteroom members
/// to reference the result of a work execution stored on a decentralized storage newtork.
/// 
/// The `ResultPtr` also references the work ID which is the hash of the `WorkPtr`
/// 
/// ## Examples
/// 
/// **NOTE: The values used in this example are dummy values**
/// 
/// ```
/// use marketplace_primitives::{ResultPtr, ResultInfo, Verify};
/// use marketplace_helpers::objects::{WU, IdHash, WorkSize};
/// use marketplace_wallet::{crypto::Crypto, Owner};
/// 
/// // Hash of WorkPtr
/// let work_id = [25u8; 32];
/// let work_size = WU::try_from(20000).unwrap();
/// 
/// // Whiteroom member
/// let owner = Owner::new_sig();
/// 
/// // Result Information
/// let result = ResultInfo::new(
///     [25u8; 32], 
///     WorkSize::build(work_size).unwrap()
/// );
/// 
/// // Address of result on decentralized storage network
/// let res_addr = [25u8; 32];
/// 
/// // Proof of whiteroom membership
/// // which must be less than the epoch's 
/// // VRF threshold
/// let crypto = Crypto::new(&owner);
/// let wr_proof = crypto
///     .attempt_wr(
///         // Job seed (see Crypto module)
///         &[25u8; 32], 
/// 
///         // VDF difficulty (see Crypto module)
///         20)
///     .unwrap();
/// 
/// let mut resultptr = ResultPtr::new(
///     work_id,
///     result,
///     wr_proof,
///     res_addr,
///     owner.as_wr_lock(),
/// );
/// 
/// // Lock result pointer to prove ownership
/// let key = owner.sign(&resultptr.id());
/// resultptr.add_auth(owner.as_wr_lock(), key, false).unwrap();
/// 
/// // Verify it's a valid Result pointer
/// assert!(resultptr.verify().is_ok());
/// 
/// ```
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
    /// Create a new `ResultPtr`
    /// 
    /// # Examples
    /// 
    /// **See ResultPtr docs**
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
    use marketplace_helpers::{functions::dum_bytes, objects::{VRF_T, WU}};
    use marketplace_wallet::{Owner, crypto::Crypto};
    use super::*;

    pub fn wr_proof(owner: &Owner, vrf_t: VRF_T) -> WRProof {
        let crypto = Crypto::new(owner);
        crypto.attempt_wr(
            &dum_bytes(), 
            functions::vdf_difficulty(
                WU::try_from(300000).unwrap(),
                vrf_t
            )
        ).unwrap()
    }

    fn res_ptr(spent: WorkSize, owner: &Owner, wr_proof: WRProof) -> ResultPtr {
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
        let wr_proof = wr_proof(&owner, [2; 32]);
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
        let wr_proof = wr_proof(&f_owner, [2; 32]);
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

        let wr_proof = wr_proof(&owner, [2; 32]);
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