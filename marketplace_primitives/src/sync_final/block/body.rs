use std::vec;
use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{objects::{ID, IdHash, WHITEROOM_SIZE, WU}};
use rs_merkle::{MerkleTree, algorithms::Sha256};

use crate::{Contract};

// Block stores list of contracts
// Empty blocks can exist
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BlockBody {
    ctrs: Vec<Contract>,
}

impl BlockBody {
    pub fn new(
        ctrs: Vec<Contract>
    ) -> Self {
        Self {
            ctrs,
        }
    }

    // Genesis Block
    pub fn genesis() -> Self {
        Self {
            ctrs: vec![Contract::genesis()]
        }  
    }
}

// Getter methods
impl BlockBody {
    // Average whiteroom size of every 
    // job contract in the block
    pub fn wr_avg(&self) -> usize {
        let (sum, count) = self.ctrs
        .iter()
        .fold((0, 0), |(mut sum, mut count), ctr| {
            if let Contract::JOB(jobctr) = ctr {
                sum += jobctr.wr_len();

                count += 1;
            }

            (sum, count)
        });

        // For an empty block, the average whiteroom
        // size is reported as WHITEROOM SIZE / 2
        // even though this is not true.
        // this is to slash the VRF threshold by half
        // which is the most a VRF threshold can be reduced.
        if count == 0 {
            return WHITEROOM_SIZE / 2
        }

        // Convert average to usize
        // The whiteroom size has a minimum size
        // any fractional part can be safely truncated
        // it does not change the average greatly
        (sum / count) as usize
    }

    // Total amount of new gdc coins in block
    pub fn total(&self) -> WU {
        self.ctrs
        .iter()
        .map(|ctr| ctr.new_coins())
        .sum()
    }

    // The id of the block is the merkle root hash
    // of all the contracts
    pub fn root(&self) -> Option<ID> {
        // Hash every contract as the leaf of the merkel tree
        let leaves: Vec<ID> = self.ctrs
            .iter()
            .map(|ctr| ctr.id())
            .collect();
        
        // Merkle tree
        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        merkle_tree.root()
    }

    // Block body
    pub fn body(&self) -> &[Contract] {
        &self.ctrs
    }

    // Number of Contracts in block
    pub fn len(&self) -> usize {
        self.ctrs.len()
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_eq, thread};
    use marketplace_helpers::objects::{AgentResult, WHITEROOM_SIZE};
use marketplace_wallet::Owner;

use crate::{BlockHeader, JobContract};
    use crate::par_exec::contract::job::tests::{resultptr, workptr};
    use super::*;

    // Valid Job contract
    pub fn jobctr() -> AgentResult<JobContract> {
        let owner = Owner::new_sig();

        // Input Block header
        let blk_hdr = BlockHeader::genesis();

        // Initialize valid Work Ptr
        let workptr = workptr(&owner, blk_hdr.id());
        let mut jobctr = JobContract::new(workptr, blk_hdr);

        // Initialize size result pointers
        // ResultPtr Initializers
        let work_id = jobctr.input().id();

        let mut handles = Vec::new();

        // Assemble Whiteroom results
        for _ in 0..WHITEROOM_SIZE {
            handles.push(thread::spawn(move || {
                let wr_owner = Owner::new_sig();

                // Result
                let result_ptr = resultptr(
                    work_id,
                    &wr_owner,
                    blk_hdr.vdf_diff()
                );

                result_ptr
            })); 
        }

        for handle in handles {
            let result_ptr = handle.join().unwrap();
            jobctr.add_result(result_ptr)?;
        }

        Ok(jobctr)
    }

    // Valid block
    #[test]
    fn valid_block() {
        let block = BlockBody::genesis();

        assert_eq!(block.total(), WU::default());
        assert_eq!(block.len(), 1);
    }

    // Test whiteroom size of an empty block is 
    // set to half of the expected whiteroom size
    #[test]
    fn avg_wr_size_of_empty_block() {
        let block = BlockBody::new(Vec::new());

        assert_eq!(block.wr_avg(), WHITEROOM_SIZE / 2)
    }


    // Test the average whiteroom size of a block
    #[test]
    fn avg_wr_size_of_block() -> AgentResult<()> {
        let block = BlockBody::new(
            vec![Contract::JOB(jobctr()?), Contract::JOB(jobctr()?), Contract::JOB(jobctr()?)]
        );

        assert_eq!(block.wr_avg(), WHITEROOM_SIZE);
        Ok(())
    }

    // An empty block body should have no contracts, no total, and no Merkle root.
    #[test]
    fn empty_body_has_empty_aggregates() {
        let body = BlockBody::new(Vec::new());

        assert_eq!(body.body().len(), 0);
        assert_eq!(body.len(), 0);
        assert_eq!(body.total(), WU::default());
        assert_eq!(body.root(), None);
    }

    // A genesis body should expose its genesis contract through the body slice.
    #[test]
    fn genesis_body_exposes_genesis_contract() {
        let body = BlockBody::genesis();

        assert_eq!(body.len(), 1);
        assert_eq!(body.body().len(), 1);
        assert!(matches!(body.body()[0], Contract::TX(_)));
        assert!(body.root().is_some());
    }

    // The body root should change when its contract list changes.
    #[test]
    fn body_root_depends_on_contracts() -> AgentResult<()> {
        let first = BlockBody::new(vec![Contract::TX(crate::TxContract::new(Vec::new()))]);
        let second = BlockBody::new(vec![Contract::genesis()]);

        assert_ne!(first.root(), second.root());
        Ok(())
    }
}