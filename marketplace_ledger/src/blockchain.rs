use marketplace_helpers::{functions, objects::{AgentResult, Epoch, ID, IdHash, StdResult, WU}};

use marketplace_primitives::{
    par_exec::{Bill, Contract, TxOpt, TxOptRef, WorkPtr}, sync_final::{Block, BlockBody, BlockHeader, blockinfo::BlockInfo},
};

use std::{collections::HashMap, panic, path::Path, sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard}};

use redb::{Database, Error, ReadableDatabase, ReadableTableMetadata, TableDefinition};

/// Store all `BlockInfo` directly and address them by their epoch
/// 
/// 
/// index -> BlockInfo
const BLOCKS: TableDefinition<Epoch, &[u8]> = TableDefinition::new("blocks");

/// Store each block index so as to get block id of corresponding epoch
/// 
/// index -> block_id
const INDEXES: TableDefinition<Epoch, &[u8]> = TableDefinition::new("indexes");

/// Store metadata for the blockcjain for example:
/// genesis block ("genesis" -> [u8; 32])
/// total goldcoin in chain ("gdc": [u8; 16])
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

use marketplace_wallet::{Key, Lock};

use crate::{Balances, State, state};

// # Blockchain
// Store list of `Blocks`
// that can be used to build state
pub struct Blockchain<B: Balances> {
    // Database
    db: RwLock<Database>,

    // UTXO State
    state: Mutex<State<B>>,

    // Store key value pair of
    // contract id and Contract to be stored
    cache: Mutex<HashMap<ID, Contract>>,
}

impl<B: Balances> Blockchain<B> {
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
            db: RwLock::new(db),
            state: Mutex::new(state),
            cache: Mutex::new(HashMap::new()) ,
        })
    }

    /// Read from blockchain database 
    pub fn db_read(&self) -> RwLockReadGuard<'_, Database> {
        self.db.read().unwrap()
    }

    /// Write to blockchain database
    pub fn db_write(&self) -> RwLockWriteGuard<'_, Database> {
        self.db.write().unwrap()
    }

    /// Interact with state by locking it
    pub fn state(&self) -> MutexGuard<'_, State<B>> {
        self.state.lock().unwrap()
    }

    /// Intearct with cache by locking it
    pub fn cache(&self) -> MutexGuard<'_, HashMap<ID, Contract>> {
        self.cache.lock().unwrap()
    }
}

// Handle modifying block body
impl<B: Balances> Blockchain<B> {
    /// Add a sub block to block body to be proposed
    pub fn add_ctrs(&self, ctrs: Vec<Contract>) {
        // Cache should always have a vector because the cache should
        // only be in None state when ctrs are being moved, which is then 
        // immediately replaced. 
        // If cache is in None state, the cache is corrupted.
        let mut cache = self.cache();

        cache.extend(
            ctrs.into_iter()
                .map(|ctr| (ctr.id(), ctr))
        ); 
    }

    // Get block body to be proposed in a new block
    // from cache
    fn get_block_body(&self) -> BlockBody {
        let mut cache = self.cache();

        BlockBody::new(
            cache.drain()
                .map(|(_, ctr)| ctr)
                .collect()
        )
    }

    /// Return a new block to be proposed to the network
    pub fn new_block(&self) -> Block {
        let blockbody = self.get_block_body();

        // Get latest block in chain which acts as new block's prev block
        let prev_hdr = self.latest_header().expect("Unable to get latest header");
        
        // Return new block
        Block::new(&prev_hdr, blockbody)
    }

    /// Remove contract from cache if it exists
    /// and return
    pub fn take_ctr(&self, id: ID) -> Option<Contract> {
        let mut cache = self.cache();

        cache.remove(&id)
    } 
}

// Methods for UtxO state
impl<B: Balances> Blockchain<B> {
    // Get gdc_balance
    pub fn gdc_balance(&self, owner: Lock) -> WU {
        let state = self.state();
        state.get_gdc_balance(owner)
    } 

    // Update state with contracts in block
    pub fn update_state_with_block(&self, block: &Block) -> AgentResult<()> {
        for ctr in block.ctrs() {
            // Remove contract from cache
            // Given that all contracts must have been added to cache 
            // during validation
            let _ = self.take_ctr(ctr.id())
                .expect("Error: Contract not found in cache");
            
            match ctr {
                Contract::JOB(jobctr) => {
                    let tx = jobctr.get_tx()?;
                    self.state().put_opts(&tx);
                },
                Contract::TX(txctr) => {
                    for tx in txctr.get_txs() {
                        self.state().put_opts(tx);
                    }
                }
            }
        }

        // Validate block header using previous block header
        let prev_hdr = self.latest_header()
            .expect("Latest block Header could not be fetched");

        block.validate(&prev_hdr)
    }

    // Try to take input UTXO's for jobcontracts
    pub fn get_workptr_utxos(
        &self, 
        workptr: &WorkPtr, 
        auth: &HashMap<ID, Key>
    ) -> AgentResult<Vec<(TxOptRef, TxOpt)>> {
        // Get goldcoin utxo list, which is the only currency that work can be done with
        let mut state = self.state();
        let balances = state.get_mut_coin_balance_list();

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
        &self, 
        utxos: Vec<(TxOptRef, TxOpt)>
    ) {
        let mut state = self.state();
        let balances = state.get_mut_coin_balance_list();

        for (utxo_ref, utxo) in utxos.into_iter().rev() {
            balances.put(utxo_ref, utxo);
        }
    }

    // Subtracts transaction inputs from the global state
    // If inputs could not be found, a double spend error is assumed.
    pub fn get_bill_utxos(&self, bill: &Bill, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Get mutable state
        let mut state = self.state();

        // Get the referenced utxos for each transaction.
        // If any issues occur, rollback the removed

        // Store each tx and corresponding utxos, so that if any 
        // rollback occurs, it can be referenced
        let mut tx_ref_opts = Vec::new();

        for tx in bill.get_txs().iter() {
            // Try to get ref opts for txs
            match state.get_ref_opts(tx, &bill.id(), auth) {
                Ok(ref_opts) => {
                    // TODO: Check ipts owners consented
                    // Store tx opts
                    tx_ref_opts.push((tx, ref_opts));
                },
                Err(e) => {
                    // Revert state changes
                    for (tx, ref_opts) in tx_ref_opts.into_iter() {
                        state.revert_ref_opts(tx, ref_opts);
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
    // Initialize blockchain database tables
    pub fn init_tables(db: &Database) -> Result<(), Error> {
        // Genesis block
        let genesis = Block::genesis();

        let write_txn = db.begin_write()?;

        // Open tables, and create them if they do not exist
        {
            // Open Blocks table
            // Store block_id -> Block
            let blocks = write_txn.open_table(BLOCKS)?;

            // Open Index table
            //  Store index -> Block id
            let mut indexes = write_txn.open_table(INDEXES)?;

            // Open metadata table
            let mut metadata = write_txn.open_table(METADATA)?;

            // If block table is empty,
            // clear other tables and initialize them
            // with genesis block.
            if blocks.is_empty()? {
                // Add genesis block as metadata and remove
                // any previous data
                metadata.insert("genesis", genesis.serialize().as_slice())?;

                // Store latest block header
                metadata.insert("latest", genesis.header().serialize().as_slice())?;

                // Clear all previous indexes if they exist
                while let Some(_) = indexes.pop_first()? {};

                // Add genesis data to index
                indexes.insert(0, genesis.id().as_slice())?;
            }
        }

        write_txn.commit()?;

        Ok(())
    }

    /// Get block from index
    pub fn get_blockinfo_by_index(&self, index: Epoch) -> StdResult<Option<BlockInfo>> {
        let read_txn;
        {
            read_txn = self.db_read().begin_read()?;
        }

        // Get Block option indicating absence or presence of the block.
        let block_option = read_txn.open_table(BLOCKS)?
            .get(index)?;

        
        Ok(
            block_option.map(
                |blkinfo_bytes| 
                BlockInfo::from_bytes(blkinfo_bytes.value())
                    .expect("Error: Block bytes are invalid")
            )
        )
    }

    /// Get blockchain genesis block
    pub fn genesis_block(&self) -> StdResult<Block> {
        let metadata; 
        {
            metadata = self.db_read()
                .begin_read()?
                .open_table(METADATA)?;
        }

        // Get genesis header, which should exist
        // or panic
        let Some(genesis) = metadata.get("genesis")?
        else {
            panic!("Illegal: Blockchain has no genesis block")
        };

        Ok(Block::from_bytes(genesis.value())?)
    }

    // Get latetst block header
    fn latest_header(&self) -> StdResult<BlockHeader> {
        let metadata;
        {
            metadata = self.db_read()
                .begin_read()?
                .open_table(METADATA)?;
        }

        let Some(header) = metadata.get("latest")? else {
            panic!("Illegal: Blockchain has no block header")
        };

        Ok(BlockHeader::from_bytes(header.value())?)
    }

    // Latest blockinfo entry in the blockchain
    pub fn latest_blockinfo(&self) -> StdResult<BlockInfo>  {
        // Latest block header index
        let index = self.latest_header()?.index();

        // Get latest block which must exist
        // which is why there is an unwrap
        Ok(
            self.get_blockinfo_by_index(index)?.unwrap()
        )
    }

    /// Returns the length of the chain which is the latest index + 1
    /// because epoch starts at 0.
    pub fn len(&self) -> StdResult<u64> {
        let len = self.latest_header()?.index();

        Ok(len + 1)
    }

    /// Confirm block exists in database
    pub fn is_block(&self, header: &BlockHeader) -> StdResult<bool> {
        let indexes; 

        {
            indexes = self.db_read()
                .begin_read()?
                .open_table(INDEXES)?;
        }

        // If header id could not be found or the id in the indexes table does not match
        // the header id, then false is returned
        if let Some(id) = indexes.get(header.index())? {
            let id: ID = id.value().try_into()?;

            if header.id() == id {
                return Ok(true)
            }
        }

        Ok(false)
    }

    /// Get seed for syncchronous finalization rounds 
    pub fn seed(&self) -> ID {
        self.latest_header().unwrap().id()
    }

    /// Total goldcoin in chain
    pub fn total_gdc(&self) -> StdResult<WU> {
        let latest_header = self.latest_header().unwrap();

        Ok(latest_header.total_gdc())
    }

    /// Check if a block id belongs to a recent block
    /// 
    /// A recent block is either the latest block or the block before it.
    pub fn is_recent(&self, id: ID) -> bool {
        let latest = self.latest_header().unwrap();

        if latest.id() == id || latest.prev() == id {
            return true
        }

        false
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
    fn add_to_db(&self, blockinfo: BlockInfo) -> StdResult<()> {
        let block = blockinfo.block();
        let index = block.header().index();

        let write_txn = self.db_write().begin_write()?;

        {
            // Update total gdc in metadata
            let mut metadata = write_txn.open_table(METADATA)?;

            metadata.insert("latest", block.header().serialize().as_slice())?;


            // Chain must have checked if block index immediately follows the
            // latest block
            let mut indexes = write_txn.open_table(INDEXES)?;
            
            indexes.insert(index, block.id().as_slice())?;


            // Update blocks with block info data
            let mut blocks = write_txn.open_table(BLOCKS)?;
            blocks.insert(index, blockinfo.serialize().as_slice())?;
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
    pub fn publish_blockinfo(&self, blockinfo: BlockInfo) -> AgentResult<()> {
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
        assert_eq!(chain.latest_header()?.id(), genesis.id());

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

    // The genesis block should be retrievable and recognized as the latest recent block.
    #[test]
    fn genesis_block_is_recent_and_retrievable() -> StdResult<()> {
        let path = test_db("genesis_recent");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let genesis = chain.genesis_block()?;

        assert_eq!(genesis.id(), Block::genesis().id());
        assert!(chain.is_recent(genesis.id()));
        assert!(!chain.is_recent([42; 32]));

        fs::remove_file(path)?;
        Ok(())
    }

    // Contracts added to the cache should be included in the next block and removed from the cache.
    #[test]
    fn cached_contracts_are_consumed_by_new_block() -> StdResult<()> {
        let path = test_db("cached_contracts");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let contract = Contract::genesis();
        let id = contract.id();
        chain.add_ctrs(vec![contract]);

        assert!(chain.cache().get(&id).is_some());

        let block = chain.new_block();

        assert_eq!(block.index(), 1);
        assert_eq!(block.header().prev(), Block::genesis().id());
        assert_eq!(block.ctrs().len(), 1);
        assert!(chain.cache().get(&id).is_none());
        assert!(chain.new_block().ctrs().is_empty());

        fs::remove_file(path)?;
        Ok(())
    }
}