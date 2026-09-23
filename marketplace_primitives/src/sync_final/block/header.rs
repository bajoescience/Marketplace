use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{ID, IdHash, VRF_T, WHITEROOM_SIZE, WU}};

use crate::{WRVote, sync_final::block::BlockBody};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct BlockHeader {
    // Epoch
    epoch: u64,

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

    // TODO: Proposer: identity that proposed this block.
    // the proposer does not contribute to the block hash.

    // TODO: Valdators: whiteroom identity that validated this block.
    // the validators do not contribute to the block hash.
}

// Associated functions
impl BlockHeader {
    pub fn genesis() -> Self {
        let genesis_body = BlockBody::genesis();

        Self {
            epoch: 0,
            ctr_merkle_root: genesis_body.root(),
            prev: [0u8; 32],
            new_gdc: genesis_body.total(),
            average: WU::single(),

            // Max 255 bit number which means a
            // whiteroom selection probability of 1
            vrf_t: [255; 32]
        }
    }

    // New Block Header instance
    // Usually created with Blockchain
    pub fn new(
        prev_header: &BlockHeader,
        body: &BlockBody, 
    ) -> Self {
        let new_avg = functions::avg_wu(
            body.total(),
            prev_header.average() 
        );

        Self {
            epoch: prev_header.epoch() + 1,
            ctr_merkle_root: body.root(),
            prev: prev_header.id(),
            new_gdc: body.total(),
            average: new_avg,

            // Calculate new VRF threshold 
            // as described in whitepaper.
            vrf_t: functions::get_vrf(
                body.wr_avg(), 
                prev_header.vrf_t()
            )
        }
    }
}

// Getter methods
impl BlockHeader {
    // Get epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

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