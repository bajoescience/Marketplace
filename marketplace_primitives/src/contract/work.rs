//! # Work Pointer
//! 
//! A module that implements the Work Pointer
//! referencing a comput task on a decentralize 
//! storage network as described by the 
//! Marketplace Whitepaper

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::format;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;
use marketplace_helpers::objects::WorkSize;
use marketplace_wallet::{Key, Lock};
use crate::{Verify};
use crate::helpers::objects::{ID, WorkAddr, AgentResult, IdHash};

// When to start executing work input
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum ExecuteTime {
    // Execute immediately
    NOW,
    // Execute after miliseconds
    AFTER(u64),
    // Execute at specified time
    AT(u64),
}

/// The WorkPtr acts like a smart pointer to a specific work to be executed
/// referenced by an address in some storage system that can
/// be accessed by other nodes.
/// 
/// The WorkPtr also contains what can be considered metadata information
/// specifying the transaction for executing the work and the time to
/// start executing.
/// 
/// It contains just enough information for nodes to start executing 
/// a work considered as a job, and transactions must be in goldcoin.
/// 
/// The WorkPtr also has authentication to prove ownership of coins/tokens
/// held in the transaction, and in the work execution.

// Pointer to work
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct WorkPtr {
    // Address of where work is stored
    work_addr: WorkAddr,

    // Work size in goldcoin
    size: WorkSize,

    // Employer Whiteroom to withdraw money to execute from
    emp_wr: Lock,

    // Specific time to execute
    do_at: ExecuteTime,

    // ID of Latest Block Header at time of publishing
    // If block header is not latest ID, the job has expired.
    blk_hdr: ID,

    // Authenticate Workptr
    // Stores <Lock.id(), Key>
    #[borsh(skip)]
    auth: HashMap<ID, Key>
}

impl WorkPtr {
    pub fn new(
        work_addr: ID, 
        size: WorkSize, 
        emp_wr: Lock,
        blk_hdr: ID,
        do_at: ExecuteTime
    ) -> Self {

        Self {
            work_addr,
            size,
            emp_wr,
            blk_hdr,
            do_at,
            auth: HashMap::new(),
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

    // Employer
    pub fn emp_wr(&self) -> &Lock {
        &self.emp_wr
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
    // force parameter will overide any previous associated keys
    pub fn add_auth(&mut self, lock: Lock, key: Key, force: bool) -> AgentResult<()> {
        match self.auth.entry(lock.id()) {
            Entry::Vacant(entry) => {
                entry.insert(key);
            },
            Entry::Occupied(mut entry) if force => {
                entry.insert(key);
            },
            Entry::Occupied(_) => {
                return Err(format!(
                    "Key already exists for Lock ID {}",
                    functions::from_bytes(&lock.id())
                ));
            },  
        }

        Ok(())
    }
}

// Verify work input
impl Verify for WorkPtr {
    fn verify(&self) -> AgentResult<()> {
        // Prove ownership of whiteroom
        let emp_wr = self.emp_wr();

        if let Some(key) = self.auth.get(&emp_wr.id()) {
            emp_wr.unlock(key, &self.id())?;
        } else {
            return Err(format!(
                "Error: Key does not exist for workptr {}",
                functions::from_bytes(&emp_wr.id()), 
            ));
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {

use marketplace_helpers::{functions::dum_bytes, objects::WU};
use marketplace_wallet::{Owner};

use crate::{BlockHeader};

use super::*;

    // Invalid work pay
    // should fail
    #[test]
    #[should_panic]
    fn verify_invalid_whiteroom_work_ipt() {
        let pay = WU::default();
        let owner = Owner::new_sig();

        let mut work_ptr = WorkPtr::new(
            dum_bytes(), 
            WorkSize::build(pay).unwrap(), 
            owner.as_wr_lock(),
            BlockHeader::genesis().id(),
            ExecuteTime::NOW
        );

        // Sign workptr and add auth proof
        let key = owner.sign(&work_ptr.id());
        work_ptr.add_auth(owner.as_lock(), key, false).unwrap();

        work_ptr.verify().unwrap();
    }

    // Verify work_ptr with valid work size
    #[test]
    fn verify_valid_work_ptr() -> AgentResult<()> {
        let pay = WU::try_from(20000).unwrap();
        let owner = Owner::new_sig();

        let mut work_ptr = WorkPtr::new(
            dum_bytes(), 
            WorkSize::build(pay).unwrap(), 
            owner.as_wr_lock(), 
            BlockHeader::genesis().id(),
            ExecuteTime::NOW
        );

        // Sign workptr and add auth proof
        let key = owner.sign(&work_ptr.id());
        work_ptr.add_auth(owner.as_wr_lock(), key, false).unwrap();

        work_ptr.verify()
    }
    
}