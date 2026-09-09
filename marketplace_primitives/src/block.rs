use std::vec;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{ID, IdHash, VRF_T, WHITEROOM_SIZE, WU}};
use rs_merkle::{MerkleTree, algorithms::Sha256};

use crate::{Contract, WRVote};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct BlockHeader {
    // Contract merkle root
    ctr_merkle_root: Option<ID>,

    // Previous block ID
    prev: ID,

    // Total amount of new gdc in block
    new_gdc: WU,

    // New average historical gdc created per block
    average: WU,

    // VRF threshold
    vrf_t: VRF_T,
}

// Associated functions
impl BlockHeader {
    pub fn genesis() -> Self {
        let genesis = Block::genesis();

        Self {
            ctr_merkle_root: genesis.root(),
            prev: [0u8; 32],
            new_gdc: WU::try_from(100).unwrap() * WU::GDC(),
            average: WU::single(),

            // Max 255 bit number which means a
            // whiteroom selection probability of 1
            vrf_t: [255; 32]
        }
    }

    // New Block Header instance
    // Usually created with Blockchain
    pub fn new(
        blk: &Block, 
        prev_header: &BlockHeader,
    ) -> Self {
        let new_avg = functions::avg_wu(
            blk.total(),
            prev_header.average() 
        );

        Self {
            ctr_merkle_root: blk.root(),
            prev: prev_header.id(),
            new_gdc: blk.total(),
            average: new_avg,

            // Calculate new VRF threshold 
            // as described in whitepaper.
            vrf_t: functions::get_vrf(
                blk.wr_avg(), 
                prev_header.vrf_t()
            )
        }
    }
}

// Getter methods
impl BlockHeader {
    // Get VDF Difficulty for blockheader
    pub fn vdf_diff(&self) -> u64 {
        // Work size is total work done on a job by
        // a whiteroom committee divided by whiteroom committee size
        let work_size = self.average / WU::try_from(WHITEROOM_SIZE as u128).unwrap();

        functions::vdf_difficulty(
            work_size, 
            self.vrf_t()
        )
    }

    // Get amount of new gdc in block
    pub fn new_gdc(&self) -> WU {
        self.new_gdc
    }

    // Get average 
    pub fn average(&self) -> WU {
        self.average
    }

    // Get VRF threshold for current epoch
    pub fn vrf_t(&self) -> VRF_T {
        self.vrf_t
    }
}

impl PartialEq for BlockHeader {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl WRVote for BlockHeader {
    fn as_vote(&self) -> ID {
        self.id()
    }
}

// Block stores list of contracts
// Empty blocks can exist
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Block {
    body: Vec<Contract>,
}

impl Block {
    /// Create a new Block from a Vec of contracts
    /// 
    /// An empty Vector can also be supplied, but it will 
    /// have a VRF_threshold that
    /// will ultimately be half the original
    pub fn new(body: Vec<Contract>) -> Self {
        Self {
            body,
        }
    }

    // Genesis Block
    pub fn genesis() -> Self {
        Self {
            body: vec![Contract::genesis()]
        }  
    }

    // Serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(&self)
            .expect("Illergal: Error Serializing")
    }
}

// Getter methods
impl Block {
    // Average whiteroom size of every 
    // job contract in the block
    pub fn wr_avg(&self) -> usize {
        let (sum, count) = self.body
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
        self.body
        .iter()
        .map(|ctr| ctr.new_coins())
        .sum()
    }

    // The id of the block is the merkle root hash
    // of all the contracts
    pub fn root(&self) -> Option<ID> {
        // Hash every contract as the leaf of the merkel tree
        let leaves: Vec<ID> = self.body
            .iter()
            .map(|ctr| ctr.id())
            .collect();
        
        // Merkle tree
        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        merkle_tree.root()
    }

    // Block body
    pub fn body(&self) -> &[Contract] {
        &self.body
    }

    // Number of Contracts in block
    pub fn len(&self) -> usize {
        self.body.len()
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_eq, thread};
    use marketplace_helpers::objects::{AgentResult, WHITEROOM_SIZE};
use marketplace_wallet::Owner;

use crate::JobContract;
    use crate::contract::tests::{resultptr, workptr};
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
        let block = Block::genesis();

        assert_eq!(block.total(), WU::default());
        assert_eq!(block.len(), 1);
    }

    // Test whiteroom size of an empty block is 
    // set to half of the expected whiteroom size
    #[test]
    fn avg_wr_size_of_empty_block() {
        let block = Block::new(Vec::new());

        assert_eq!(block.wr_avg(), WHITEROOM_SIZE / 2)
    }


    // Test the average whiteroom size of a block
    #[test]
    fn avg_wr_size_of_block() -> AgentResult<()> {
        let block = Block::new(
            vec![Contract::JOB(jobctr()?), Contract::JOB(jobctr()?), Contract::JOB(jobctr()?)]
        );

        assert_eq!(block.wr_avg(), WHITEROOM_SIZE);
        Ok(())
    }
}