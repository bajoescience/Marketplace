mod header;
mod body;

use borsh::{BorshDeserialize, BorshSerialize};
pub use header::BlockHeader;
pub use body::BlockBody;

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
}

// Getter functions
impl Block {
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
}


#[cfg(test)]
mod tests {
    use super::*;
}