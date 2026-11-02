//! # Work Pointer
//! 
//! A module that implements the Work Pointer
//! referencing a compute task on a decentralized
//! storage network as described in the 
//! Marketplace Whitepaper

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::objects::WorkSize;
use marketplace_wallet::Lock;
use crate::par_exec::TxOptRef;
use crate::helpers::objects::{ID, WorkAddr};

/// When to start executing work
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum ExecuteTime {
    // Execute immediately
    NOW,

    // Execute after miliseconds
    AFTER(u64),

    // Execute at specified time
    AT(u64),
}

/// # Work Pointer `WorkPtr`
/// 
/// `WorkPtr` is a pointer like type that references a specific work to be executed
/// by it's address in a decentralized storage system which is accessible by other nodes.
/// 
/// It is sent to the network by an employer to initiate a `Job` 
/// 
/// The `WorkPtr` can be locked to prove ownership.
/// 
/// ## Example
/// 
/// **NOTE: The values used in this example are dummy values**
/// 
/// ```
/// use marketplace_primitives::{
///     par_exec::{ExecuteTime, TxOptRef, WorkPtr},
///     sync_final::BlockHeader,
///     Verify,
/// };
/// use marketplace_helpers::objects::{WU, IdHash, WorkSize};
/// use marketplace_wallet::Owner;
/// 
/// // Address of Work in Decentralized staorage
/// let work_address = [20u8; 32];
/// 
/// // Work size of one gdc
/// let work_size = WU::GDC();
/// 
/// // Time to execute work
/// let do_at = ExecuteTime::NOW;
/// 
/// // Employer
/// let employer = Owner::new_sig();
/// 
/// // ID of latest block header
/// let blk_hdr = BlockHeader::genesis();
/// 
/// // Create work pointer instance
/// let mut workptr = WorkPtr::new(
///     work_address,
///     WorkSize::build(work_size).unwrap(),
///     vec![TxOptRef::new([255u8; 32], 0)],
///     employer.as_wr_lock(),
///     blk_hdr.id(),
///     do_at
/// );
/// 
/// // Verify it's a valid Work Pointer
/// assert!(workptr.verify().is_ok());
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct WorkPtr {
    // Address of where work is stored
    work_addr: WorkAddr,

    // Work size in goldcoin
    size: WorkSize,

    // Employer UTXOs
    ipts: Vec<TxOptRef>,

    // Remainder Account
    // to receive any remainder amount
    remainder: Lock,

    // Specific time to execute
    do_at: ExecuteTime,

    // ID of Latest Block Header at time of publishing
    // If block header is not latest ID, the job has expired.
    blk_hdr: ID,
}

impl WorkPtr {
    /// Create a new `WorkPtr` instance
    /// 
    /// # Example
    /// 
    /// **NOTE: The values used in this example are dummy values**
    /// 
    /// ```
    ///     use marketplace_primitives::{
    ///         par_exec::{ExecuteTime, TxOptRef, WorkPtr},
    ///         sync_final::BlockHeader,
    ///         Verify,
    ///     };
    /// use marketplace_helpers::objects::{WU, IdHash, WorkSize};
    /// use marketplace_wallet::Owner;
    /// 
    /// // Address of Work in Decentralized staorage
    /// let work_address = [20u8; 32];
    /// 
    /// // Work size of one gdc
    /// let work_size = WU::GDC();
    ///  
    /// // Time to execute work
    /// let do_at = ExecuteTime::NOW;
    /// 
    /// // Employer
    /// let employer = Owner::new_sig();
    /// 
    /// // Ipt transaction output
    /// let ipts = vec![TxOptRef::new([255u8; 32], 0)];
    /// 
    /// // ID of latest block header
    /// let blk_hdr = BlockHeader::genesis();
    /// 
    /// // Create work pointer instance
    /// let mut workptr = WorkPtr::new(
    ///     work_address,
    ///     WorkSize::build(work_size).unwrap(),
    ///     ipts,
    ///     employer.as_wr_lock(),
    ///     blk_hdr.id(),
    ///     do_at
    /// );
    /// 
    /// // Verify it's a valid Work Pointer
    /// assert!(workptr.verify().is_ok());
    /// ```
    pub fn new(
        work_addr: ID, 
        size: WorkSize, 
        ipts: Vec<TxOptRef>,
        remainder: Lock,
        blk_hdr: ID,
        do_at: ExecuteTime
    ) -> Self {

        Self {
            work_addr,
            size,
            ipts,
            blk_hdr,
            do_at,
            remainder
        }
    }
}

// Getter methods
impl WorkPtr {
    pub fn work_addr(&self) -> &ID {
        &self.work_addr
    }

    pub fn work_size(&self) -> WorkSize {
        self.size
    }

    // Inputs
    pub fn ipts(&self) -> &Vec<TxOptRef> {
        &self.ipts
    }

    // Remainder account
    pub fn remainder(&self) -> Lock {
        self.remainder
    }

    // Get Block header ID
    pub fn blk_hdr(&self) -> &ID {
        &self.blk_hdr
    }

    pub fn do_at(&self) -> &ExecuteTime {
        &self.do_at
    }
}

// Setter methods
impl WorkPtr {
}

// Verify sturcture of work input

// The verification of UTXOs is done by the state
// when the workptr is passed through it.

#[cfg(test)]
mod tests {

use std::vec;

use marketplace_helpers::{objects::WU};
use marketplace_wallet::Owner;

use super::*;
    // Work pointer getters should return the values supplied at construction.
    #[test]
    fn work_pointer_getters() {
        let owner = Owner::new_sig();

        let work_addr = [1u8; 32];
        let blk_hdr = [2u8; 32];
        let size = WorkSize::build(WU::try_from(20000).unwrap()).unwrap();
        let work_ptr = WorkPtr::new(
            work_addr,
            size,
            vec![
                TxOptRef::new([255; 32], 0)
                ],
            owner.as_lock(),
            blk_hdr,
            ExecuteTime::AFTER(10),
        );

        assert_eq!(work_ptr.work_addr(), &work_addr);
        assert_eq!(work_ptr.work_size().into(), size.into());
        assert_eq!(
            work_ptr.ipts(), 
            &vec![
                TxOptRef::new([255; 32], 0)
            ]
        );
        assert_eq!(work_ptr.blk_hdr(), &blk_hdr);
        assert!(matches!(work_ptr.do_at(), ExecuteTime::AFTER(10)));
    }

}