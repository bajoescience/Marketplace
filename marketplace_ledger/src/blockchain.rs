use marketplace_helpers::{functions, objects::{AgentResult, Epoch, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{
    par_exec::{Bill, Contract, TxOpt, TxOptRef, Whiteroom, WorkPtr}, sync_final::{Block, BlockBody, Validated, blockinfo::BlockInfo},
};

use std::{collections::HashMap, panic, path::Path};

use redb::{Database, Error, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

/// Store all `BlockInfo` directly and address them by their id
/// 
/// 
/// block_id -> Block
const BLOCKS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blocks");

/// Store each block index so as to get block id of corresponding epoch
/// 
/// index -> block_id
const INDEXES: TableDefinition<Epoch, &[u8]> = TableDefinition::new("indexes");

/// Store list of validators that authenticate block
/// 
/// index -> Validators
const AUTHS: TableDefinition<Epoch, &[u8]> = TableDefinition::new("auths");

/// Store metadata for the blockcjain for example:
/// genesis block ("genesis" -> [u8; 32])
/// total goldcoin in chain ("gdc": [u8; 16])
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

use marketplace_wallet::{Key, voting::EXPECTED_VOTERS};

use crate::{Balances, State, state};

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

// # Blockchain
// Store list of `Blocks`
// that can be used to build state
pub struct Blockchain<B: Balances> {
    // Database
    db: Database,

    // UTXO State
    state: State<B>,

    // Store key value pair of
    // contract id and Contract to be stored
    cache: Option<HashMap<ID, Contract>>,
}

impl<B: Balances> Blockchain<B> {
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

            // Open Index table
            //  Store index -> Block id
            let mut indexes = write_txn.open_table(INDEXES)?;

            // Open metadata table
            let mut metadata = write_txn.open_table(METADATA)?;

            // If block table is empty,
            // clear other tables and initialize them
            // with genesis block.
            if blocks.is_empty()? {
                // Add genesis metadata and remove
                // any previous data
                metadata.insert("genesis", genesis_id.as_slice())?;
                metadata.insert("gdc", genesis.total_gdc().as_slice())?;

                // Clear all previous indexes if they exist
                while let Some(_) = indexes.pop_first()? {};

                // Add genesis data to index
                indexes.insert(0, genesis_id.as_slice())?;

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
    pub fn init(path: impl AsRef<Path>, state: State<B>) -> StdResult<Self> {
        // Initialize blockchain database
        let db = Database::create(path)?;

        Self::init_tables(&db)?;

        Ok(Self {
            db,
            state,
            cache: Some(HashMap::new()),
        })
    }
}

// Handle modifying block body
impl<B: Balances> Blockchain<B> {
    /// Add a sub block to block body to be proposed
    pub fn add_ctrs(&mut self, ctrs: Vec<Contract>) {
        // Cache should always have a vector because the cache should
        // only be in None state when ctrs are being moved, which is then 
        // immediately replaced. 
        // If cache is in None state, the cache is corrupted.
        let cache = self.cache.as_mut()
            .expect("Illegal: Cache should not be in None state");

        cache.extend(
            ctrs.into_iter()
                .map(|ctr| (ctr.id(), ctr))
        ); 
    }

    // Get block body to be proposed in a new block
    // from cache
    fn get_block_body(&mut self) -> BlockBody {
        let ctrs = self.cache
            .take()
            .expect("Illegal: Cache should not be in None state");

        self.cache = Some(HashMap::new());

        BlockBody::new(
            ctrs.into_iter()
                .map(|(_, ctr)| ctr)
                .collect()
        )
    }

    /// Return a new block to be proposed to the network
    pub fn new_block(&mut self) -> Block {
        let blockbody = self.get_block_body();

        // Get latest block in chain which acts as new block's prev block
        let prev_block = self.latest_block().expect("Unable to get latest block");
        
        // Return new block
        Block::new(prev_block.header(), blockbody)
    }

    /// Remove contract from cache if it exists
    /// and return
    pub fn take_ctr(&mut self, id: ID) -> Option<Contract> {
        let cache = self.cache.as_mut()
            .expect("Illegal: Cache should not be in None state");

        cache.remove(&id)
    }

    /// Get contract from cache if it exists
    pub fn get_ctr(&self, id: &ID) -> Option<&Contract> {
        let cache = self.cache
            .as_ref()
            .expect("Illegal: Cache should not be in None state");

        cache.get(id)
    } 
    
}

// Methods for UtxO state
impl<B: Balances> Blockchain<B> {
    pub fn state(&self) -> &State<B> {
        &self.state
    }

    // Update state with contracts in block
    pub fn update_state_with_block(&mut self, block: &Block) -> AgentResult<()> {
        for ctr in block.ctrs() {
            // Remove contract from cache
            // Given that all contracts must have been added to cache 
            // during validation
            let _ = self.take_ctr(ctr.id())
                .expect("Error: Contract not found in cache");
            
            match ctr {
                Contract::JOB(jobctr) => {
                    let tx = jobctr.get_tx()?;
                    self.state.put_opts(&tx);
                },
                Contract::TX(txctr) => {
                    for tx in txctr.get_txs() {
                        self.state.put_opts(tx);
                    }
                }
            }
        }

        // Validate block header using previous block header
        let prev_block = self.latest_block()
            .expect("Latest block could not be fetched");

        block.validate(prev_block.header())
    }

    // Try to take input UTXO's for jobcontracts
    pub fn get_workptr_utxos(
        &mut self, 
        workptr: &WorkPtr, 
        auth: &HashMap<ID, Key>
    ) -> AgentResult<Vec<(TxOptRef, TxOpt)>> {
        // Get goldcoin utxo list, which is the only currency that work can be done with
        let balances = self.state.get_mut_coin_balance_list();

        // List of utxos
        let mut ref_opts: Vec<(TxOptRef, TxOpt)> = Vec::new();

        for utxo_ref in workptr.ipts() {
            // Ensure utxo exists
            let Some(utxo) = balances.get(*utxo_ref) else {
                return Err(format!(
                    "Error: Tx Output {} not found.",
                    functions::from_bytes(&utxo_ref.id())
                ))
            };

            match state::confirm_utxo(&utxo, &workptr.id(), auth) {
                Ok(_) => {
                    ref_opts.push((utxo_ref.clone(), utxo))
                },
                Err(e) => {
                    // Revert removed utxos
                    for (utxo_ref, utxo) in ref_opts {
                        balances.put(utxo_ref, utxo);
                    }
                    return Err(e);
                }
            };
        }

        Ok(ref_opts)
    }

    // Return utxo taken by a job back to state
    pub fn revert_jobctr_utxos(
        &mut self, 
        utxos: Vec<(TxOptRef, TxOpt)>
    ) {
        let balances = self.state.get_mut_coin_balance_list();

        for (utxo_ref, utxo) in utxos.into_iter().rev() {
            balances.put(utxo_ref, utxo);
        }
    }

    // Subtracts transaction inputs from the global state
    // If inputs could not be found, a double spend error is assumed.
    pub fn get_bill_utxos(&mut self, bill: &Bill, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Get the referenced utxos for each transaction.
        // If any issues occur, rollback the removed

        // Store each tx and corresponding utxos, so that if any 
        // rollback occurs, it can be referenced
        let mut tx_ref_opts = Vec::new();

        for tx in bill.get_txs().iter() {
            // Try to get ref opts for txs
            match self.state.get_ref_opts(tx, &bill.id(), auth) {
                Ok(ref_opts) => {
                    // TODO: Check ipts owners consented
                    // Store tx opts
                    tx_ref_opts.push((tx, ref_opts));
                },
                Err(e) => {
                    // Revert state changes
                    for (tx, ref_opts) in tx_ref_opts.into_iter() {
                        self.state.revert_ref_opts(tx, ref_opts);
                    }
                    return Err(e)
                }
                
            };
        }

        Ok(())
    }
}

// Getter methods for database
impl<B: Balances> Blockchain<B> {
    /// Get block from index
    pub fn get_block_by_id(&self, id: &[u8]) -> StdResult<Option<Block>> {
        let read_txn = self.db.begin_read()?;

        // Get Block option indicating absence or presence of the block.
        let block_option = read_txn.open_table(BLOCKS)?
            .get(id)?;

        
        Ok(
            block_option.map(
                |blk_bytes| 
                Block::from(blk_bytes.value())
                    .expect("Error: Block bytes are invalid")
            )
        )
    }

    /// Get auth by index
    pub fn get_auth_by_index(&self, index: Epoch) -> StdResult<Option<Whiteroom<Validated, WHITEROOM_MAX>>> {
        let read_txn = self.db.begin_read()?;

        let auth_option = read_txn.open_table(AUTHS)?
            .get(index)?;

        Ok(
            auth_option.map(
                |auth_bytes|
                Whiteroom::from(auth_bytes.value())
                    .expect("Error: Auth bytes are invalid")
            )
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

    /// Get blockinfo by index
    /// 
    /// Note that this method returns None for the genesis index of 0
    /// 
    /// It also returns None for indexes not found in the blockchain DB
    pub fn get_blockinfo_by_index(&self, index: Epoch) -> StdResult<Option<BlockInfo>> {
        let read_txn = self.db.begin_read()?;

        // Genesis block info does not exist
        if index == 0 {
            return Ok(None)
        }

        let auth_option = self.get_auth_by_index(index)?;

        // Get block id from epoch table if it exists
        let Some(block_id) = read_txn.open_table(INDEXES)?
            .get(index)?
        else {
            return Ok(None)
        };

        // Get block
        let block_option = self.get_block_by_id(block_id.value())?;

        let (Some(block), Some(auth)) = (block_option, auth_option) else {
            return Ok(None)
        };

        Ok(Some(BlockInfo::new(block, auth)))
    
    }

    // Latest blockinfo entry in the blockchain
    fn latest_epoch_entry(&self) -> StdResult<(Epoch, ID)>  {
        let indexes = self.db
            .begin_read()?
            .open_table(INDEXES)?;

        // Epoch table must at least contain genesis block id for epoch 0.
        let (index, block_id_bytes) = indexes.last()?
            .expect("Illegal: Blocks table should not be empty and must contain a genesis block");

        Ok((
            index.value(),
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

        let block = self.get_block_by_id(blk_id.as_slice())?
            .expect("Error: Blockchain should have at least genesis block");

        Ok(block)
    }

    /// Get seed for syncchronous finalization rounds 
    pub fn seed(&self) -> ID {
        self.latest_block().unwrap().header().id()
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
    pub fn iter(&self, index: u64) -> impl Iterator<Item = BlockInfo> {
        BlockChainIterator {
            index,
            chain: &self,
        }
    }
}

// Setter methods
impl<B: Balances> Blockchain<B> {
    // Add to file
    fn add_to_db(&mut self, blockinfo: BlockInfo) -> StdResult<()> {
        // Sepearte blockinfo into block and auth
        let (block, auth) = blockinfo.destroy();
        let index = block.header().index();

        let write_txn = self.db.begin_write()?;

        {
            // Update total gdc in metadata
            let mut metadata = write_txn.open_table(METADATA)?;
            let gdc = block.header().total_gdc();

            metadata.insert("gdc", gdc.inner().to_be_bytes().as_slice())?;


            // Chain must have checked if block index immediately follows the
            // latest block
            let mut indexes = write_txn.open_table(INDEXES)?;
            
            indexes.insert(index, block.id().as_slice())?;


            // Update blocks and auths tables with block info data
            let mut blocks = write_txn.open_table(BLOCKS)?;
            blocks.insert(block.id().as_slice(), block.serialize().as_slice())?;

            let mut auths = write_txn.open_table(AUTHS)?;
            auths.insert(index, auth.serialize().as_slice())?;
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
        self.update_state_with_block(blockinfo.block())?;

        // Add block to db
        self.add_to_db(blockinfo)
            .expect("Error: Block could not be added to chain");

        Ok(())
    }
}

// Iterator struct for blockchain
// to iterate through blocks
struct BlockChainIterator<'a, B: Balances> {
    index: u64,
    chain: &'a Blockchain<B>
}

impl<'a, B: Balances> Iterator for BlockChainIterator<'a, B> {
    type Item = BlockInfo;

    // Returns next block
    fn next(&mut self) -> Option<Self::Item> {
        let block_option = self.chain
            .get_blockinfo_by_index(self.index)
            .expect("Error iterating through blockchain");

        self.index += 1;

        block_option
    }
}

#[cfg(test)]
mod tests {

use crate::state;

// These tests exercise the blockchain lifecycle: initialization, block publication,
// block lookup, persistence, iteration, validation, and recent-block tracking.
use super::*;
    use std::{assert_eq, fs, path::PathBuf, vec};
    use marketplace_primitives::par_exec::{Bill, Tx, TxContract, TxIdentifier, TxOpt, TxOptRef};
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

        let bill = Bill::new(vec![
            Tx::new(
                0, 
                TxIdentifier::COIN, 
                ipt.clone(), 
                opt.clone()
            )
        ]);

        let key = owner.sign(&bill.id());

        let mut txctr = TxContract::new(bill);

        txctr.add_auth(
            owner.as_lock(), 
            key,
        );

        Contract::TX(txctr)
        
    }

    // Test a new chain has one block: genesis block
    // test the single block is the genesis block
    #[test]
    fn init_blockchain() -> StdResult<()> {
        let path = test_db("init");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;

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
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;

        assert!(chain.iter(chain.len()?).next().is_none());
        assert!(chain.get_blockinfo_by_index(1)?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }

    // Return no block when a requested block ID is absent from the database.
    #[test]
    fn returns_none_for_unknown_block_id() -> StdResult<()> {
        let path = test_db("unknown_block_id");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;

        assert!(chain.get_block_by_id(&[42u8; 32])?.is_none());

        fs::remove_file(path)?;
        Ok(())
    }
}