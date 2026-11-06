use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{CONTRACT_WHITEROOM_SIZE, ID, IdHash, VRF_T, WU}};

use crate::{sync_final::block::BlockBody};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BlockHeader {
    // Block number starting from 0 (genesis block)
    index: u64,

    // Contract merkle root
    ctr_merkle_root: Option<ID>,

    // Previous block ID
    prev: ID,

    // Total amount of gdc in chain
    total_gdc: WU,

    // New average historical gdc created per block
    average: WU,

    // VRF threshold
    vrf_t: VRF_T,
}

// Associated functions
impl BlockHeader {
    pub fn genesis() -> Self {
        let genesis_body = BlockBody::genesis();

        Self {
            index: 0,
            ctr_merkle_root: genesis_body.root(),
            prev: [0u8; 32],

            // Amount of new gdc
            total_gdc: WU::GDC(),
            
            average: WU::single(),

            // Max 255 bit number which means a
            // whiteroom selection probability of 1
            vrf_t: [255; 32],
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
            index: prev_header.index() + 1,
            ctr_merkle_root: body.root(),
            prev: prev_header.id(),
            total_gdc: body.total() + prev_header.total_gdc,
            average: new_avg,

            // Calculate new VRF threshold 
            // as described in whitepaper.
            vrf_t: functions::get_vrf(
                body.wr_avg(), 
                prev_header.vrf_t()
            ),
        }
    }
}

// Getter methods
impl BlockHeader {
    // Get epoch
    pub fn index(&self) -> u64 {
        self.index
    }

    // Get VDF Difficulty for blockheader
    pub fn vdf_diff(&self) -> u64 {
        // Work size is total work done on a job by
        // a whiteroom committee divided by whiteroom committee size
        let work_size = self.average / WU::try_from(CONTRACT_WHITEROOM_SIZE as u128).unwrap();

        functions::vdf_difficulty(
            work_size, 
            self.vrf_t()
        )
    }

    // Get total amount of gdc in blockchain
    pub fn total_gdc(&self) -> WU {
        self.total_gdc
    }

    // Get average 
    pub fn average(&self) -> WU {
        self.average
    }

    // Get VRF threshold for current epoch
    pub fn vrf_t(&self) -> VRF_T {
        self.vrf_t
    }

    // Get previous block header
    pub fn prev(&self) -> ID {
        self.prev
    }
}

// Setter methods
impl BlockHeader {
   
}

impl PartialEq for BlockHeader {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

#[cfg(test)]
mod tests {
    use marketplace_helpers::objects::IdHash;

    use crate::sync_final::block::BlockBody;

    use super::*;

    // Genesis headers should initialize the chain at epoch zero.
    #[test]
    fn genesis_header_has_initial_values() {
        let header = BlockHeader::genesis();

        assert_eq!(header.index(), 0);
        assert_eq!(header.prev(), [0u8; 32]);
        assert_eq!(header.total_gdc(), WU::GDC());
        assert_eq!(header.average(), WU::single());
        assert_eq!(header.vrf_t(), [255u8; 32]);
    }

    // A new header should reference its previous header and body-derived values.
    #[test]
    fn new_header_references_previous_header_and_body() {
        let previous = BlockHeader::genesis();
        let body = BlockBody::new(Vec::new());
        let header = BlockHeader::new(&previous, &body);

        assert_eq!(header.index(), previous.index() + 1);
        assert_eq!(header.prev(), previous.id());
        assert_eq!(header.total_gdc(), body.total() + previous.total_gdc());
        assert_eq!(
            header.vrf_t(),
            functions::get_vrf(body.wr_avg(), previous.vrf_t())
        );
    }

    // Rebuilding a header from the same previous header and body should be deterministic.
    #[test]
    fn header_construction_is_deterministic() {
        let previous = BlockHeader::genesis();
        let body = BlockBody::new(Vec::new());

        let first = BlockHeader::new(&previous, &body);
        let second = BlockHeader::new(&previous, &body);

        assert_eq!(first, second);
        assert_eq!(first.id(), second.id());
        assert_eq!(first.vdf_diff(), second.vdf_diff());
    }
}