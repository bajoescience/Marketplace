use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{Block, BlockHeader, Contract, Verify};

use std::{collections::{VecDeque}, format, path::Path};

use redb::{Database, Error, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

/// Store blocks directly and address them by their id
/// 
/// block_id -> Block
const BLOCKS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blocks");

/// Store each block epoch so as to get block id of corresponding epoch
/// 
/// epoch -> block_id
const EPOCHS: TableDefinition<u64, &[u8]> = TableDefinition::new("epochs");

/// Store metadata for the blockcjain for example:
/// genesis block ("genesis" -> [u8; 32])
/// total goldcoin in chain ("gdc": [u8; 16])
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

                // Insert genesis block
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

        // Ensure block epoch is the next chain epoch
        let Ok(chain_len) = self.len() else {
            return Err(format!(
                "Error: Chain length could not be fetched"
            ));
        };

        if block.header().epoch() != chain_len {
            return Err(format!(
                "Error: Block of epoch {} is the expected block, not {}",
                chain_len,
                block.header().epoch()
            ))
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

        Ok(
            Some(Block::from(block_bytes.value())?)
        )
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

    
    fn latest_epoch_entry(&self) -> StdResult<(u64, ID)>  {
        let epoch = self.db
            .begin_read()?
            .open_table(EPOCHS)?;

        // Epoch table must at least contain genesis block id for epoch 0.
        let (epoch, block_id_bytes) = epoch.last()?
            .expect("Illegal: Blocks table should not be empty");

        Ok((
            epoch.value(),
            block_id_bytes.value().try_into()?
        ))
    }

    /// Returns the length of the chain which is the latest epoch + 1
    /// because epoch starts at 0.
    pub fn len(&self) -> StdResult<u64> {
        let (len, _) = self.latest_epoch_entry()?;

        Ok(len + 1)
    }

    /// Return the latest block
    pub fn latest_block(&self) -> StdResult<Block> {
        // Block id
        let (_, blk_id) = self.latest_epoch_entry()?;

        // Get block from BLOCKS table useing id
        let block = self.db
            .begin_read()?
            .open_table(BLOCKS)?;

        // Get latest block bytes
        let block_bytes = block.get(blk_id.as_slice())?
            .expect("Illegal: Latest block is not in chain");

        Ok(Block::from(block_bytes.value())?)
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

            // Chain must have checked if block epoch immediately follows the
            // latest block
            let mut epochs = write_txn.open_table(EPOCHS)?;
            
            epochs.insert(blk.header().epoch(), blk.id().as_slice())?;

            // Update blocks
            let mut blocks = write_txn.open_table(BLOCKS)?;
            blocks.insert(blk.id().as_slice(), blk.serialize().as_slice())?;
        }

        write_txn.commit()?;

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
    use std::{assert_eq, fs, path::PathBuf, vec};
    use marketplace_primitives::{Tx, TxContract, TxIO, TxIdentifier};
use marketplace_wallet::Owner;

    fn test_db(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "marketplace_ledger_{name}_{}_{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_file(&path);
        path
    }

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

    // Test a new chain has one block: genesis block
    // test the single block is the genesis block
    #[test]
    fn init_blockchain() -> StdResult<()> {
        let path = test_db("init");
        let chain = Blockchain::init(&path)?;

        // Only one block
        assert_eq!(chain.len()?, 1);

        // Single block is genesis
        let genesis = Block::genesis();
        assert_eq!(chain.total_gdc()?, WU::GDC());
        assert_eq!(chain.latest_block()?.id(), genesis.id());

        fs::remove_file(path)?;
        Ok(())
    }

    // Add an empty block to the blockchain
    #[test]
    fn add_empty_block_to_blockchain() -> StdResult<()> {
        let path = test_db("add_empty_block");
        let mut chain = Blockchain::init(&path)?;

        let block = Block::new(&BlockHeader::genesis(), vec![]);

        chain.add_block(block)?;

        assert_eq!(chain.len()?, 2);

        fs::remove_file(path)?;
        Ok(())
    }

    // TODO: Test adding a normal block to the chain
    #[test]
    fn add_normal_block_to_blockchain() -> StdResult<()> {
        // Owner in contract
        let owner = Owner::new_sig();

        let path = test_db("add_normal_block");
        let mut chain = Blockchain::init(&path)?;

        let block = Block::new(
            &BlockHeader::genesis(), 
            vec![ctr(&owner, &owner)]
        );

        chain.add_block(block)?;

        assert_eq!(chain.len()?, 2);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn rejects_block_for_an_existing_epoch() -> StdResult<()> {
        let path = test_db("rejects_existing_epoch");
        let mut chain = Blockchain::init(&path)?;

        chain.add_block(Block::new(&BlockHeader::genesis(), vec![]))?;
        let duplicate_epoch = Block::new(&BlockHeader::genesis(), vec![]);

        let error = chain.add_block(duplicate_epoch).unwrap_err();
        assert!(error.contains("expected block"));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn rejects_invalid_transaction_contract() -> StdResult<()> {
        let path = test_db("rejects_invalid_transaction");
        let chain = Blockchain::init(&path)?;
        let invalid_contract = Contract::TX(TxContract::new(vec![Tx::genesis()]));
        let block = Block::new(&BlockHeader::genesis(), vec![invalid_contract]);

        let error = chain.validate_block(&block).unwrap_err();
        assert!(!error.is_empty());

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn persists_blocks_and_total_gdc_after_reopen() -> StdResult<()> {
        let path = test_db("persists_blocks");
        let block_id;

        {
            let mut chain = Blockchain::init(&path)?;
            let block = Block::new(&BlockHeader::genesis(), vec![]);
            block_id = block.id();
            chain.add_block(block)?;

            assert_eq!(chain.len()?, 2);
            assert_eq!(chain.total_gdc()?, WU::GDC());
        }

        let chain = Blockchain::init(&path)?;
        assert_eq!(chain.len()?, 2);
        assert_eq!(chain.latest_block()?.id(), block_id);
        assert!(chain.find_block(&block_id)?.is_some());

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn keeps_only_the_two_latest_header_ids() -> StdResult<()> {
        let path = test_db("latest_headers");
        let mut chain = Blockchain::init(&path)?;

        let first = Block::new(&BlockHeader::genesis(), vec![]);
        let first_id = first.id();
        chain.add_block(first)?;

        let second = Block::new(&chain.latest_block()?.header().clone(), vec![]);
        let second_id = second.id();
        chain.add_block(second)?;

        let third = Block::new(&chain.latest_block()?.header().clone(), vec![]);
        let third_id = third.id();
        chain.add_block(third)?;

        assert_eq!(chain.latest_hdrs().len(), 2);
        assert_eq!(chain.latest_hdrs()[0], second_id);
        assert_eq!(chain.latest_hdrs()[1], third_id);
        assert_ne!(chain.latest_hdrs()[0], first_id);

        fs::remove_file(path)?;
        Ok(())
    }
}