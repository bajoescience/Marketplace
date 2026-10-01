mod header;
mod body;

use std::{ collections::{HashMap, hash_map::Entry}, format};

use borsh::{BorshDeserialize, BorshSerialize};
pub use header::BlockHeader;
pub use body::BlockBody;
use marketplace_helpers::{ functions, objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock};

use crate::{Contract, Proposer, Verify};

/// # Block
/// 
/// `Block` is a list of contracts finalized by the network during an epoch.
/// 
/// When `Contracts` are finalized in blocks, they are considered dead state, and immutable.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Block {
    header: BlockHeader,
    body: BlockBody,

    // Prove ownership of proposer or validator
    // in blockheader
    #[borsh(skip)]
    auth: HashMap<ID, Key>,
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
            body,
            auth: HashMap::new(),
        }
    }

    // Genesis block
    pub fn genesis() -> Self {
        Block { 
            header: BlockHeader::genesis(),
            body: BlockBody::genesis(),
            auth: HashMap::new(),
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

// Setter methods
impl Block {
    /// Add a proposal to a block, and sign the block.
    /// 
    /// The owner must be the same used to create the proposal
    pub fn add_proposer(&mut self, proposer: Proposer) {
        self.header.add_proposer(proposer)
    }

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

// Validation
impl Block {
    /// Validate the block uses the same previous header
    pub fn validate(&self, prev_header: &BlockHeader) -> AgentResult<()> {
        // This should be the block header if the previous header is the input of
        // the function.
        let blk_hdr = BlockHeader::new(prev_header, &self.body);

        // Compare headers
        if self.id() != blk_hdr.id() {
            return Err(format!(
                "Error: Block header does not match expected"
            ))
        }

        Ok(())
    }
}

// Verify block must have proposal
// before being considered by the rest of the network
impl Verify for Block {
    fn verify(&self) -> AgentResult<()> {
        let header = self.header();

        // Ensure proposer exists in block
        // Ensure proposer exists in block
        let Some(proposer) = header.proposer() else {
            return Err(format!(
                "Error: Proposer not found in block"
            ))
        };

        // Verify proposer using previous block header
        proposer.verify(&header.prev())?;

        // Get lock using public key in proposer
        let lock = Lock::SIG(
            functions::hash(proposer.pk())
        );

        // Unlock lock to prove ownership
        if let Some(key) = self.auth.get(&lock.id()) {
            lock.unlock(key, &self.id())?;
        } else {
            return Err(format!(
                "Error: Key does not exist for proposer {}",
                functions::from_bytes(&lock.id()), 
            ));
        };

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::assert_eq;

use marketplace_helpers::objects::WU;

use super::*;

    // Test gdc of block
    #[test]
    pub fn genesis_gdc() {
        let genesis = Block::genesis();

        assert_eq!(genesis.header().new_gdc(), WU::GDC());
    }

    // A newly constructed block should expose its previous-header linkage and body.
    #[test]
    fn new_block_exposes_header_and_body() {
        let previous = BlockHeader::genesis();
        let block = Block::new(&previous, Vec::new());

        assert_eq!(block.body().len(), 0);
        assert_eq!(block.header().epoch(), previous.epoch() + 1);
        assert_eq!(block.header().prev(), previous.id());
        assert_eq!(block.id(), block.header().id());
    }

    // Serialized blocks should deserialize back to an equivalent block.
    #[test]
    fn block_serialization_round_trip() -> Result<(), std::io::Error> {
        let block = Block::genesis();
        let restored = Block::from(&block.serialize())?;

        assert_eq!(restored.id(), block.id());
        assert_eq!(restored.body().len(), block.body().len());
        assert_eq!(restored.gdc(), block.gdc());
        Ok(())
    }

    // Block validation should accept the header used to construct the block.
    #[test]
    fn block_validates_against_previous_header() -> AgentResult<()> {
        let previous = BlockHeader::genesis();
        let block = Block::new(&previous, Vec::new());

        block.validate(&previous)
    }

    // Block validation should reject a different previous header.
    #[test]
    fn block_rejects_wrong_previous_header() {
        let previous = BlockHeader::genesis();
        let block = Block::new(&previous, Vec::new());
        let wrong_previous = BlockHeader::new(&previous, &BlockBody::new(Vec::new()));

        let err = block.validate(&wrong_previous).unwrap_err();
        assert_eq!(err, "Error: Block header is the prev header");
    }
}