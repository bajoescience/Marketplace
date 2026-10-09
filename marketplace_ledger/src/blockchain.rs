use marketplace_helpers::{functions, objects::{AgentResult, Epoch, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{
    Block, Contract, Proposal, Verify, 
    sync_final::{blockinfo::BlockInfo},
};
use marketplace_wallet::voting::EXPECTED_VOTERS;

use std::{format, panic, path::Path};

use redb::{Database, Error, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

/// Store all `BlockInfo` directly and address them by their id
/// 
/// **Note: Only `Genesis Block` is stored as a lone `Block`, every other `Block` is stored as `BlockInfo`
/// which includes the following:**
/// 
/// - Block 
/// 
/// - Proposer
/// 
/// - Validators
/// 
/// block_id -> BlockInfo
const BLOCKS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blocks");

/// Store each block epoch so as to get block id of corresponding epoch
/// 
/// epoch -> block_id
const EPOCHS: TableDefinition<Epoch, &[u8]> = TableDefinition::new("epochs");

/// Store metadata for the blockcjain for example:
/// genesis block ("genesis" -> [u8; 32])
/// total goldcoin in chain ("gdc": [u8; 16])
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

// # Blockchain
// Store list of `Blocks`
// that can be used to build state
pub struct Blockchain {
    // Database
    db: Database,
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
            db,
        })
    }

    // Validate a contract
    fn validate_contract(&self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::TX(txctr) => txctr.verify(),
            Contract::JOB(jobctr) => {

                // Get blockinfo referenced by Job contract
                let Some(_) = self.get_blockinfo_by_id(&jobctr.get_blk_hdr().id())
                    .expect("Error encountered finding block from database") 
                else {
                    return Err(format!(
                        "Error: Block {} does not exist in Blockchain",
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

        // Validate block header using previous block
        let prev_block = self.latest_block()
            .expect("Latest block could not be fetched");

        block.validate(prev_block.header())
    }
}

// Getter methods
impl Blockchain {
    /// Find blockInfo using block ID
    /// 
    /// Note: This only works for non genesis id
    /// it would return `None` for genesis id, 
    /// 
    /// use the `genesis_block()` method instead to get
    /// genesis block
    pub fn get_blockinfo_by_id(&self, id: &[u8]) -> StdResult<Option<BlockInfo>> {
        let read_txn = self.db.begin_read()?;

        // Get genesis header, which should exist
        // or panic
        let Some(genesis) = read_txn
            .open_table(METADATA)?
            .get("genesis")?
        else {
            panic!("Illegal: Blockchain has no genesis block")
        };

        // Block id must not be genesis id
        if id == genesis.value() {
            return Ok(None);
        }

        // If block does not exist, return None
        let Some(block_bytes) = read_txn
            .open_table(BLOCKS)?
            .get(id)?
        else {
            return Ok(None);
        };

        Ok(
            Some(BlockInfo::from(block_bytes.value())?)
        )
    }

    /// Get blockchain genesis block
    pub fn genesis_block(&self) -> StdResult<Block> {
        let read_txn = self.db.begin_read()?;

        // Get genesis header, which should exist
        // or panic
        let Some(genesis) = read_txn
            .open_table(METADATA)?
            .get("genesis")?
        else {
            panic!("Illegal: Blockchain has no genesis block")
        };

        // If genesis block does not exist, panic
        let Some(block_bytes) = read_txn
            .open_table(BLOCKS)?
            .get(genesis.value())?
        else {
            panic!("Illegal: Genesis block not found");
        };

        Ok(Block::from(block_bytes.value())?)
    }

    /// Get block by epoch
    pub fn get_blockinfo_by_epoch(&self, epoch: u64) -> StdResult<Option<BlockInfo>> {
        let read_txn = self.db.begin_read()?;

        // Get block id from epoch table if it exists
        let Some(block_id) = read_txn.open_table(EPOCHS)?
            .get(epoch)?
        else {
            return Ok(None)
        };

        self.get_blockinfo_by_id(block_id.value())
    }

    // Latest blockinfo entry in the blockchain
    fn latest_epoch_entry(&self) -> StdResult<(Epoch, ID)>  {
        let epoch = self.db
            .begin_read()?
            .open_table(EPOCHS)?;

        // Epoch table must at least contain genesis block id for epoch 0.
        let (epoch, block_id_bytes) = epoch.last()?
            .expect("Illegal: Blocks table should not be empty and must contain a genesis block");

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
        let (epoch, blk_id) = self.latest_epoch_entry()?;

        if epoch == 0 {
            return Ok(self.genesis_block()?)
        }

        let block = self.get_blockinfo_by_id(blk_id.as_slice())?
            .expect("Error: Blockchain should have at least genesis block");

        Ok(block.block().clone())
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
    pub fn iter(&self, epoch: u64) -> impl Iterator<Item = BlockInfo> {
        BlockChainIterator {
            epoch,
            chain: &self,
        }
    }
}

// Setter methods
impl Blockchain {
    // Add to file
    fn add_to_db(&mut self, blockinfo: BlockInfo) -> StdResult<()> {
        let block = blockinfo.block();
        let write_txn = self.db.begin_write()?;

        {
            // Update total gdc in metadata
            let mut metadata = write_txn.open_table(METADATA)?;
            let mut gdc = self.total_gdc()?;
            gdc = gdc + block.header().new_gdc();

            metadata.insert("gdc", gdc.inner().to_be_bytes().as_slice())?;

            // Chain must have checked if block epoch immediately follows the
            // latest block
            let mut epochs = write_txn.open_table(EPOCHS)?;
            
            epochs.insert(block.header().epoch(), block.id().as_slice())?;

            // Update blocks with block info
            let mut blocks = write_txn.open_table(BLOCKS)?;
            blocks.insert(block.id().as_slice(), blockinfo.serialize().as_slice())?;
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
    pub fn publish_blockinfo(&mut self, blockinfo: BlockInfo) -> AgentResult<()> {

        // Confirm and validate block
        self.validate_block(blockinfo.block())?;

        // Add block to db
        self.add_to_db(blockinfo)
            .expect("Error: Block could not be added to chain");

        Ok(())
    }
}

// Store block and subuser count
struct ProposalBlock {
    block: Option<Block>,
    subusers: u64
}

impl ProposalBlock {
    fn new() -> Self {
        Self {
            block: None,
            subusers: 0,
        }
    }

    fn update(&mut self, proposal: &Proposal, subusers: u64) {
        // Update subuser count
        self.subusers += subusers;

        let Proposal { block, .. } = proposal;

        match &self.block {
            Some(current_block) => {
                // If new block has more new gdc than current block, replace block
                if block.header().new_gdc() > current_block.header().new_gdc() {
                    self.block = Some(block.clone());
                }
            },
            None => {
                self.block = Some(block.clone())
            }
        };
    } 
}

// Iterator struct for blockchain
// to iterate through blocks
struct BlockChainIterator<'a> {
    epoch: u64,
    chain: &'a Blockchain
}

impl<'a> Iterator for BlockChainIterator<'a> {
    type Item = BlockInfo;

    // Returns next block
    fn next(&mut self) -> Option<Self::Item> {
        let block_option = self.chain
            .get_blockinfo_by_epoch(self.epoch)
            .expect("Error iterating through blockchain");

        self.epoch += 1;

        block_option
    }
}

#[cfg(test)]
mod tests {

// These tests exercise the blockchain lifecycle: initialization, block publication,
// block lookup, persistence, iteration, validation, and recent-block tracking.
use super::*;
    use std::{assert_eq, fs, path::PathBuf, vec};
    use marketplace_primitives::{Tx, TxContract, TxIdentifier, tx::{TxOpt, TxOptRef}};
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

        let ipt = Some(vec![TxOptRef::new(
            [255; 32], 
            0,
        )]);

        let opt = Some(vec![
            TxOpt::new(
                owner1.as_lock(), 
                WU::try_from(30000).unwrap(),
            ), 
            TxOpt::new(
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

    // Return no blocks when iteration begins beyond the current chain tip.
    #[test]
    fn iterator_returns_none_after_the_tip() -> StdResult<()> {
        let path = test_db("iterator_after_tip");
        let chain = Blockchain::init(&path)?;

        assert!(chain.iter(chain.len()?).next().is_none());
        assert!(chain.get_blockinfo_by_epoch(1)?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }

    // Return no block when a requested block ID is absent from the database.
    #[test]
    fn returns_none_for_unknown_block_id() -> StdResult<()> {
        let path = test_db("unknown_block_id");
        let chain = Blockchain::init(&path)?;

        assert!(chain.get_blockinfo_by_id(&[42u8; 32])?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }
}