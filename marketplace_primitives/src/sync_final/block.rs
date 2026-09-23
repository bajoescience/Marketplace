mod header;
mod body;

use borsh::{BorshDeserialize, BorshSerialize};
pub use header::BlockHeader;
pub use body::BlockBody;
use marketplace_helpers::objects::{ID, IdHash};

use crate::Contract;

/// # Block
/// 
/// `Block` is a list of contracts finalized by the network during an epoch.
/// 
/// When `Contracts` are finalized in blocks, they are considered dead state, and immutable.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Block {
    header: BlockHeader,
    body: BlockBody,
}

impl Block {
    /// Create a new Block from a Vec of contracts
    /// 
    /// An empty Vector can also be supplied, but it will 
    /// have a VRF_threshold that
    /// will ultimately be half the original
    pub fn new(prev_blk_hdr: &BlockHeader, ctrs: Vec<Contract>) -> Self {
        // Get block body
        let body = BlockBody::new(ctrs);

        // Get block header
        let header = BlockHeader::new(prev_blk_hdr, &body);

        Self {
            header,
            body
        }
    }

    // Genesis block
    pub fn genesis() -> Self {
        Block { 
            header: BlockHeader::genesis(),
            body: BlockBody::genesis(), 
        }
    }

    /// Create block from block bytes
    pub fn from(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }
}

// Getter functions
impl Block {
    // Block id is the hash of it's blockheader
    pub fn id(&self) -> ID {
        self.header().id()
    }

    // Fetch block header
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    // Fetch contracts in block
    pub fn body(&self) -> &[Contract] {
        self.body.body()
    }

    // Serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(&self)
            .expect("Illegal: Error Serializing")
    }

    // Total created gdc in bytes
    pub fn gdc(&self) -> [u8; 16] {
        self.header().new_gdc().inner().to_be_bytes()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
}