use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{Block, BlockHeader, Contract, Verify};

use std::{collections::{VecDeque}, format, fs, path::Path, todo};

use redb::{Database, Error, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

// Store blocks directly and address them by their hash
const BLOCKS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blocks");

// Store each block epoch so as to get block id of corresponding epoch
const EPOCHS: TableDefinition<u64, &[u8]> = TableDefinition::new("epochs");

// Store metadata for the blockcjain for example:
// chain_tip ("latest" -> [u8; 32])
// genesis block ("genesis" -> [u8; 32])
// total goldcoin in chain ("gdc": [u8; 16])
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

// Blockchain
// Store list of block headers
// The block and headers are kept in a file
// while only the headers are kept in memory
pub struct Blockchain {
    // Latest Block headers
    latest: VecDeque<ID>,

    // Database
    db: Database
}

impl Blockchain {
    // Initialize blockchain database tables
    pub fn init_tables(db: &Database) -> Result<(), Error> {
        // Genesis block
        let genesis = Block::genesis();
        let genesis_id = genesis.id();

        let write_txn = db.begin_write()?;

        // Open tables, and create them if they do not exist
        {
            // Open Blocks table
            // Store block_id -> Block
            let mut blocks = write_txn.open_table(BLOCKS)?;

            // Open Epoch table
            //  Store epoch -> Block id
            let mut epochs = write_txn.open_table(EPOCHS)?;

            // Open metadata table
            let mut metadata = write_txn.open_table(METADATA)?;

            // If block table is empty,
            // clear other tables and initialize them
            // with genesis block.
            if blocks.is_empty()? {
                // Add genesis metadata and remove
                // any previous data
                metadata.insert("genesis", genesis_id.as_slice())?;
                metadata.insert("gdc", genesis.gdc().as_slice())?;

                // Clear all previous epochs if they exist
                while let Some(_) = epochs.pop_first()? {};

                // Add genesis data to epoch
                epochs.insert(0, genesis_id.as_slice())?;

                // Clear all previous blocks if they exist
                while let Some(_) = blocks.pop_first()? {};
                blocks.insert(genesis_id.as_slice(), genesis.serialize().as_slice())?;
            }
        }

        write_txn.commit()?;

        Ok(())
    }

    /// Initialize a `Blockchain`
    /// 
    /// using the path of the blockchain database file
    /// This is used in the initialization 
    /// of the Marketplace State
    pub fn init(path: impl AsRef<Path>) -> StdResult<Self> {
        // Store latest headers
        let latest = VecDeque::with_capacity(2);

        // Initialize blockchain database
        let db = Database::create(path)?;

        Self::init_tables(&db)?;

        Ok(Self {
            latest,
            db
        })
    }

    // Validate a contract
    fn validate_contract(&self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::TX(txctr) => txctr.verify(),
            Contract::JOB(jobctr) => {

                // Get block referenced by Job contract
                let Some(_) = self.find_block(&jobctr.get_blk_hdr().id())
                    .expect("Error encountered finding block from database") 
                else {
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
    // Find block using ID
    pub fn find_block(&self, id: &[u8]) -> StdResult<Option<Block>>  {
        let read_txn = self.db.begin_read()?;

        // If block does not exist, return None
        let Some(block_bytes) = read_txn
            .open_table(BLOCKS)?
            .get(id)?
        else {
            return Ok(None);
        };

        Ok(Some(Block::from(block_bytes.value()).expect("Error converting bytes to block")))
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

    /// Return the latest block and the blocks epoch
    pub fn latest_block(&self) -> StdResult<(u64, Block)>  {
        let epoch = self.db
            .begin_read()?
            .open_table(EPOCHS)?;

        // Blocks table must at least contain genesis block.
        let (epoch, block_bytes) = epoch.last()?
            .expect("Illegal: Blocks table should not be empty");

        Ok((
            epoch.value(),
            Block::from(block_bytes.value()).expect("Error converting bytes to block")
        ))
    }

    // Total goldcoin in existence
    pub fn total_gdc(&self) -> StdResult<WU> {
        let metadata = self.db
            .begin_read()?
            .open_table(METADATA)?;

        // "gdc" should be defined in the db
        // as part of initialization
        let bytes = metadata.get("gdc")?
            .expect("Illegal: gdc entry should exist");

        let int = u128::from_be_bytes(bytes.value().try_into()?);

        Ok(WU::try_from(int).unwrap())
    }
}

// Setter methods
impl Blockchain {
    // Add to file
    fn add_to_file(&mut self, blk: Block) -> StdResult<()> {
        let write_txn = self.db.begin_write()?;

        {
            // Update total gdc in metadata
            let mut metadata = write_txn.open_table(METADATA)?;
            let mut gdc = self.total_gdc()?;
            gdc = gdc + blk.header().new_gdc();

            metadata.insert("gdc", gdc.inner().to_be_bytes().as_slice())?;

            // TODO: Chain must have checked if block epoch immediately follows the
            // latest block
            let mut epochs = write_txn.open_table(EPOCHS)?;
            
            epochs.insert(blk.header().epoch(), blk.id().as_slice())?;

            // Update blocks
            let mut blocks = write_txn.open_table(BLOCKS)?;
            blocks.insert(blk.id().as_slice(), blk.serialize().as_slice())?;
        }

        Ok(())
    }


    // Append header to chain
    fn append_to_chain(&mut self, hdr: &BlockHeader) {

        // Update latest header
        if self.latest.len() == 2 {
            self.latest.pop_front();
        }

        self.latest.push_back(hdr.id());
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

        // TODO: Store block in file
        self.append_to_chain(block.header());

        // Add block to db
        self.add_to_file(block)
            .expect("Error: Block could not be added to chain");
        
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

    // Test Transaction Contract where owner sends money to another and 
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
    fn add_empty_block_to_blockchain() -> StdResult<()> {
        let mut chain = Blockchain::init("blockchain.db")?;

        let block = Block::new(&BlockHeader::genesis(), vec![]);

        chain.add_block(block)?;

        assert_eq!(chain.len(), 2);
        Ok(())
    }

    // TODO: Test adding a normal block to the chain

    // TODO: Test adding a faulty block to the chain
}