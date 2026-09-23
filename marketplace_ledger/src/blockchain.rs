use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{Block, Contract, Verify};

use std::{format, path::Path};

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
        // Initialize blockchain database
        let db = Database::create(path)?;

        Self::init_tables(&db)?;

        Ok(Self {
            db
        })
    }

    // Validate a contract
    fn validate_contract(&self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::TX(txctr) => txctr.verify(),
            Contract::JOB(jobctr) => {

                // Get block referenced by Job contract
                let Some(_) = self.get_block_by_id(&jobctr.get_blk_hdr().id())
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

        // Ensure block references the last block
        let current = self.latest_block()
            .expect("Error: Latest block does not exist");

        if block.header().prev() != current.id() {
            return Err(format!(
                "Error: Block {} does not reference previous block {}",
                functions::from_bytes(&block.id()),
                functions::from_bytes(&current.id())
            ));
        }

        Ok(())
    }
}

// Getter methods
impl Blockchain {
    // Find block using ID
    pub fn get_block_by_id(&self, id: &[u8]) -> StdResult<Option<Block>> {
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

    /// Get block by epoch
    pub fn get_block_by_epoch(&self, epoch: u64) -> StdResult<Option<Block>> {
        let read_txn = self.db.begin_read()?;

        // Get block id from epoch table if it exists
        let Some(block_id) = read_txn.open_table(EPOCHS)?
            .get(epoch)?
        else {
            return Ok(None)
        };

        self.get_block_by_id(block_id.value())
    }

    // Latest block entry in the blockchain
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

    /// Returns the latest block
    /// 
    /// Also: the prev block id can be gotten from the latest
    /// block header
    pub fn latest_block(&self) -> StdResult<Block> {
        // Block id
        let (_, blk_id) = self.latest_epoch_entry()?;

        let block = self.get_block_by_id(blk_id.as_slice())?
            .expect("Error: Blockchain should have at least genesis block");

        Ok(block)
    }

    /// Total goldcoin in chain
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

    /// Check if a block id belongs to a recent block
    /// 
    /// A recent block is either the latest block or the block before it.
    pub fn is_recent(&self, id: ID) -> StdResult<bool> {
        let latest = self.latest_block()?;

        if latest.id() == id || latest.header().prev() == id {
            return Ok(true)
        }

        Ok(false)
    }

    /// # Blockchain Iterator
    /// 
    /// Iterates through the `Blockchain` by taking as input an epoch, 
    /// and returning the epoch `Block` followed by the next until
    /// when the `Blockchain` reaches the tip, the iterator returns `None`
    pub fn iter(&self, epoch: u64) -> impl Iterator<Item = Block> {
        BlockChainIterator {
            epoch,
            chain: &self,
        }
    }
}

// Setter methods
impl Blockchain {
    // Add to file
    fn add_to_db(&mut self, blk: Block) -> StdResult<()> {
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

    /// Add a new block to the Blockchain
    /// A new BlockHeader instance is created and stored
    /// in memory by the Blockchain
    /// 
    /// This method takes a Block from the mempool and
    /// adds it to the chain
    pub fn add_block(&mut self, block: Block) -> AgentResult<()> {

        // Confirm and validate block
        self.validate_block(&block)?;

        // Add block to db
        self.add_to_db(block)
            .expect("Error: Block could not be added to chain");

        Ok(())
    }

    // TODO: Add received finalized block to mempool
}

// Iterator struct for blockchain
// to iterate through blocks
struct BlockChainIterator<'a> {
    epoch: u64,
    chain: &'a Blockchain
}

impl<'a> Iterator for BlockChainIterator<'a> {
    type Item = Block;

    // Returns next block
    fn next(&mut self) -> Option<Self::Item> {
        let block_option = self.chain
            .get_block_by_epoch(self.epoch)
            .expect("Error iterating through blockchain");

        self.epoch += 1;

        block_option
    }
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

        let block = Block::new(Block::genesis().header(), vec![]);

        chain.add_block(block)?;

        assert_eq!(chain.len()?, 2);

        fs::remove_file(path)?;
        Ok(())
    }

    // Add a valid transaction block to the chain.
    #[test]
    fn add_normal_block_to_blockchain() -> StdResult<()> {
        // Owner in contract
        let owner = Owner::new_sig();

        let path = test_db("add_normal_block");
        let mut chain = Blockchain::init(&path)?;

        let block = Block::new(
            Block::genesis().header(), 
            vec![ctr(&owner, &owner)]
        );

        chain.add_block(block)?;

        assert_eq!(chain.len()?, 2);

        fs::remove_file(path)?;
        Ok(())
    }

    // Reject a block whose epoch is already present in the chain.
    #[test]
    fn rejects_block_for_an_existing_epoch() -> StdResult<()> {
        let path = test_db("rejects_existing_epoch");
        let mut chain = Blockchain::init(&path)?;

        chain.add_block(Block::new(Block::genesis().header(), vec![]))?;
        let duplicate_epoch = Block::new(Block::genesis().header(), vec![]);

        let error = chain.add_block(duplicate_epoch).unwrap_err();
        assert!(error.contains("expected block"));

        fs::remove_file(path)?;
        Ok(())
    }

    // Reject a block containing a transaction contract that fails verification.
    #[test]
    fn rejects_invalid_transaction_contract() -> StdResult<()> {
        let path = test_db("rejects_invalid_transaction");
        let chain = Blockchain::init(&path)?;
        let invalid_contract = Contract::TX(TxContract::new(vec![Tx::genesis()]));
        let block = Block::new(Block::genesis().header(), vec![invalid_contract]);

        let error = chain.validate_block(&block).unwrap_err();
        assert!(!error.is_empty());

        fs::remove_file(path)?;
        Ok(())
    }

    // Preserve blocks and chain metadata when the database is reopened.
    #[test]
    fn persists_blocks_and_total_gdc_after_reopen() -> StdResult<()> {
        let path = test_db("persists_blocks");
        let block_id;

        {
            let mut chain = Blockchain::init(&path)?;
            let block = Block::new(Block::genesis().header(), vec![]);
            block_id = block.id();
            chain.add_block(block)?;

            assert_eq!(chain.len()?, 2);
            assert_eq!(chain.total_gdc()?, WU::GDC());
        }

        let chain = Blockchain::init(&path)?;
        assert_eq!(chain.len()?, 2);
        assert_eq!(chain.latest_block()?.id(), block_id);
        assert!(chain.get_block_by_id(&block_id)?.is_some());

        fs::remove_file(path)?;
        Ok(())
    }

    // Iterate from the genesis epoch through every block in ascending order.
    #[test]
    fn iterates_from_genesis_to_tip() -> StdResult<()> {
        let path = test_db("iterates_from_genesis");
        let mut chain = Blockchain::init(&path)?;

        chain.add_block(Block::new(chain.latest_block()?.header(), vec![]))?;
        chain.add_block(Block::new(chain.latest_block()?.header(), vec![]))?;

        let block_ids: Vec<ID> = chain.iter(0).map(|block| block.id()).collect();
        let expected_ids: Vec<ID> = (0..chain.len()?)
            .map(|epoch| chain.get_block_by_epoch(epoch).unwrap().unwrap().id())
            .collect();

        assert_eq!(block_ids, expected_ids);
        assert_eq!(block_ids.len(), 3);

        fs::remove_file(path)?;
        Ok(())
    }

    // Start iteration at a later epoch and stop when the chain tip is reached.
    #[test]
    fn iterator_can_start_at_a_nonzero_epoch() -> StdResult<()> {
        let path = test_db("iterator_nonzero_start");
        let mut chain = Blockchain::init(&path)?;

        chain.add_block(Block::new(chain.latest_block()?.header(), vec![]))?;
        chain.add_block(Block::new(chain.latest_block()?.header(), vec![]))?;

        let epoch_one_id = chain.get_block_by_epoch(1)?.unwrap().id();
        let epoch_two_id = chain.get_block_by_epoch(2)?.unwrap().id();
        let blocks: Vec<Block> = chain.iter(1).collect();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id(), epoch_one_id);
        assert_eq!(blocks[1].id(), epoch_two_id);

        fs::remove_file(path)?;
        Ok(())
    }

    // Return no blocks when iteration begins beyond the current chain tip.
    #[test]
    fn iterator_returns_none_after_the_tip() -> StdResult<()> {
        let path = test_db("iterator_after_tip");
        let chain = Blockchain::init(&path)?;

        assert!(chain.iter(chain.len()?).next().is_none());
        assert!(chain.get_block_by_epoch(1)?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }

    // Return the expected block for an existing epoch and none for a missing epoch.
    #[test]
    fn gets_blocks_by_epoch() -> StdResult<()> {
        let path = test_db("get_block_by_epoch");
        let mut chain = Blockchain::init(&path)?;
        let next_block = Block::new(chain.latest_block()?.header(), vec![]);
        let next_block_id = next_block.id();
        chain.add_block(next_block)?;

        assert_eq!(chain.get_block_by_epoch(0)?.unwrap().id(), Block::genesis().id());
        assert_eq!(chain.get_block_by_epoch(1)?.unwrap().id(), next_block_id);
        assert!(chain.get_block_by_epoch(2)?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }
}