mod state;
mod mempool;

use std::{collections::{HashMap, VecDeque}, format, fs, todo, vec};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};

use marketplace_primitives::{Block, BlockHeader, Contract, Verify};

pub use state::{State};
pub use mempool::Mempool;

// Blockchain
// Store list of block headers
// The block and headers are kept in a file
// while only the headers are kept in memory
pub struct Blockchain {
    // Store BlockHeader ID as key
    headers: HashMap<ID, BlockHeader>,

    // Latest Block headers
    latest: VecDeque<ID>,

    // Total amount of gdc in existence
    total: WU,

    blockfile: Vec<Block>,
}

impl Blockchain {
    /// Initialize a new Blockchain
    /// This is used in the initialization 
    /// of the Marketplace State
    pub fn new() -> Self {
        // Add genesis header
        let headers = HashMap::new();
        let latest = VecDeque::with_capacity(2);

        let genesis_hdr = BlockHeader::genesis(); 

        let mut chain = Self { 
            headers,
            latest,
            total: genesis_hdr.new_gdc(),
            blockfile: vec![Block::genesis()],
        };

        // Update latest and add headers to chain
        chain.append_to_chain(genesis_hdr);

        chain
    }

    // Validate a contract
    fn validate_contract(&self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::TX(txctr) => txctr.verify(),
            Contract::JOB(jobctr) => {
                // Get block header referenced by contract
                let Some(_) = self.find_hdr(&jobctr.get_blk_hdr().id()) else {
                return Err(format!(
                    "Error: Block Header {} does not exist in Blockchain",
                        functions::from_bytes(&jobctr.get_blk_hdr().id()) 
                    ))
                };

                // Verify Job contract
                jobctr.verify()
            },
        }
    }

    // Validate a block
    pub fn validate_block(&self, block: &Block) -> AgentResult<()> {
        for ctr in block.body() {
            self.validate_contract(ctr)?;
        }

        Ok(())
    }
}

// Getter methods
impl Blockchain {
    // Find header using ID
    pub fn find_hdr(&self, id: &ID) -> Option<&BlockHeader> {
        self.headers.get(id)   
    }

    // latest block_headers
    pub fn latest_hdrs(&self) -> &VecDeque<ID> {
        &self.latest
    } 

    // Latest header
    pub fn latest_hdr_id(&self) -> &ID {
        self.latest
            .back()
            .expect(
                "Error: Blockchain has no Blocks, genesis block is missing!"
            )
    }

    pub fn latest_hdr(&self) -> &BlockHeader {
        self.headers
            .get(self.latest_hdr_id())
            .unwrap()
    }

    // Get average from a blockheader using blockheader ID
    pub fn get_average_using(&self, hdr_id: &ID) -> AgentResult<WU> {
        let Some(hdr) = self.find_hdr(hdr_id) else {
            return Err(format!(
                "Error: Header of id {} could not be found",
                functions::from_bytes(hdr_id)
            ))
        };

        Ok(hdr.average())
    }

    // Get newest average
    pub fn current_average(&self) -> WU {
        let hdr = self.latest_hdr();
        hdr.average()
    }

    // Create a new Block header using Block
    pub fn header_from(&self, block: &Block) -> BlockHeader {
        BlockHeader::new(
            block, 
            self.latest_hdr()
        )
    }

    // Total goldcoin in existence
    pub fn total_gdc(&self) -> WU {
        self.total
    }
}

// Setter methods
impl Blockchain {
    // Add to file
    fn add_to_file(&self, block: Block) -> AgentResult<()> {
        let bytes = block.serialize();

        let Ok(_) = fs::write("blockfile.dat", bytes) else {
            return Err(format!(
                "Error: Error adding Block to Blockfile"
            ));
        };

        Ok(())
    }

    // Read from file
    fn read_from_file(&self) -> AgentResult<()> {
        todo!()
    }

    // Append header to chain
    fn append_to_chain(&mut self, hdr: BlockHeader) {

        // Update latest header
        if self.latest.len() == 2 {
            self.latest.pop_front();
        }

        self.latest.push_back(hdr.id());

        // Add header to chain
        self.headers.insert(hdr.id(), hdr);
    }

    /// Add a new block to the Blockchain
    /// A new BlockHeader instance is created and stored
    /// in memory by the Blockchain
    /// 
    /// This method takes a Block from the mempool and
    /// adds it to the chain
    pub fn add_block(&mut self, block: Block) -> AgentResult<()> {

        // Confirm and validate block
        self.validate_block(&block)?;

        // Generate block header
        let header = self.header_from(&block);

        // TODO: Store block in file
        self.append_to_chain(header);
        self.blockfile.push(block);

        Ok(())
    }

    // TODO: Add received finalized block to mempool
}


#[cfg(test)]
mod tests {

use super::*;
    use std::{assert_eq, vec};
    use marketplace_primitives::{Tx, TxContract, TxIO, TxIdentifier};
use marketplace_wallet::Owner;

    // Test Transactions where owner sends money to another and 
    // the rest to himself
    fn ctr(owner: &Owner, owner1: &Owner) -> Contract {

        let ipt = Some(vec![TxIO::new(
            owner.as_lock(), 
            WU::GDC(),
        )]);

        let opt = Some(vec![
            TxIO::new(
                owner1.as_lock(), 
                WU::try_from(30000).unwrap(),
            ), 
            TxIO::new(
                owner.as_lock(), 
                WU::GDC() - WU::try_from(30000).unwrap(),
            )
        ]);

        let txs = vec![
            Tx::new(
                0, 
                TxIdentifier::COIN, 
                ipt.clone(), 
                opt.clone()
            )
        ];

        let mut txctr = TxContract::new(txs);
        let key = owner.sign(&txctr.id());

        txctr.add_auth(
            owner.as_lock(), 
            key, 
            false
        ).unwrap();

        Contract::TX(txctr)
        
    }

    // Add an empty block to the blockchain
    #[test]
    fn add_empty_block_to_blockchain() -> AgentResult<()> {
        let mut chain = Blockchain::new();

        let block = Block::new(vec![]);

        chain.add_block(block)?;

        assert_eq!(chain.headers.len(), 2);
        Ok(())
    }

    // TODO: Test adding a normal block to the chain

    // TODO: Test adding a faulty block to the chain
}