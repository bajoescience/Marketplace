use marketplace_helpers::{
    functions,
    objects::{AgentResult, Epoch, IdHash, StdResult, ID, WU},
};

use marketplace_primitives::{
    par_exec::{Bill, Contract, TxOpt, TxOptRef, WorkPtr},
    sync_final::{blockinfo::BlockInfo, Block, BlockBody, BlockHeader},
};

use std::{
    collections::HashMap,
    panic,
    path::Path,
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

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

use crate::{state, Balances, State};

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
            cache: Mutex::new(HashMap::new()),
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

        cache.extend(ctrs.into_iter().map(|ctr| (ctr.id(), ctr)));
    }

    // Get block body to be proposed in a new block
    // from cache
    fn get_block_body(&self) -> BlockBody {
        let mut cache = self.cache();

        BlockBody::new(cache.drain().map(|(_, ctr)| ctr).collect())
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

    // Update state with contracts in a finalized block
    // block passed in must have been confirmed
    pub fn update_state_with_block(&self, block: &Block) -> AgentResult<()> {
        for ctr in block.ctrs() {
            // Remove contract from cache
            // Given that all contracts must have been added to cache
            // during validation
            let _ = self
                .take_ctr(ctr.id())
                .expect("Error: Contract not found in cache");

            match ctr {
                Contract::JOB(jobctr) => {
                    let tx = jobctr.get_tx()?;
                    self.state().put_opts(&tx);
                }
                Contract::TX(txctr) => {
                    for tx in txctr.get_txs() {
                        self.state().put_opts(tx);
                    }
                }
            }
        }

        // Validate block header using previous block header
        let prev_hdr = self
            .latest_header()
            .expect("Latest block Header could not be fetched");

        block.validate(&prev_hdr)
    }

    // Try to take input UTXO's for jobcontracts
    pub fn get_workptr_utxos(
        &self,
        workptr: &WorkPtr,
        auth: &HashMap<ID, Key>,
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
                ));
            };

            match state::confirm_utxo(&utxo, &workptr.id(), auth) {
                Ok(_) => ref_opts.push((utxo_ref.clone(), utxo)),
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
    pub fn revert_jobctr_utxos(&self, utxos: Vec<(TxOptRef, TxOpt)>) {
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
                }
                Err(e) => {
                    // Revert state changes
                    for (tx, ref_opts) in tx_ref_opts.into_iter() {
                        state.revert_ref_opts(tx, ref_opts);
                    }
                    return Err(e);
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
                while let Some(_) = indexes.pop_first()? {}

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
        let block_option = read_txn.open_table(BLOCKS)?.get(index)?;

        Ok(block_option.map(|blkinfo_bytes| {
            BlockInfo::from_bytes(blkinfo_bytes.value()).expect("Error: Block bytes are invalid")
        }))
    }

    /// Get blockchain genesis block
    pub fn genesis_block(&self) -> StdResult<Block> {
        let metadata;
        {
            metadata = self.db_read().begin_read()?.open_table(METADATA)?;
        }

        // Get genesis header, which should exist
        // or panic
        let Some(genesis) = metadata.get("genesis")? else {
            panic!("Illegal: Blockchain has no genesis block")
        };

        Ok(Block::from_bytes(genesis.value())?)
    }

    // Get latetst block header
    pub fn latest_header(&self) -> StdResult<BlockHeader> {
        let metadata;
        {
            metadata = self.db_read().begin_read()?.open_table(METADATA)?;
        }

        let Some(header) = metadata.get("latest")? else {
            panic!("Illegal: Blockchain has no block header")
        };

        Ok(BlockHeader::from_bytes(header.value())?)
    }

    // Latest blockinfo entry in the blockchain
    pub fn latest_blockinfo(&self) -> StdResult<BlockInfo> {
        // Latest block header index
        let index = self.latest_header()?.index();

        // Get latest block which must exist
        // which is why there is an unwrap
        Ok(self.get_blockinfo_by_index(index)?.unwrap())
    }

    /// Returns the length of the chain which is the latest index + 1
    /// because epoch starts at 0.
    pub fn len(&self) -> Epoch {
        self.latest_header().unwrap().index() + 1
    }

    /// Confirm block exists in database
    pub fn is_block(&self, header: &BlockHeader) -> StdResult<bool> {
        let indexes;

        {
            indexes = self.db_read().begin_read()?.open_table(INDEXES)?;
        }

        // If header id could not be found or the id in the indexes table does not match
        // the header id, then false is returned
        if let Some(id) = indexes.get(header.index())? {
            let id: ID = id.value().try_into()?;

            if header.id() == id {
                return Ok(true);
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
            return true;
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
        // If no header exists, the blockcinfo is invalid
        // and should have been taken care of earlier in the code
        let header = blockinfo.header().unwrap();
        let index = header.index();

        let write_txn = self.db_write().begin_write()?;

        {
            // Update total gdc in metadata
            let mut metadata = write_txn.open_table(METADATA)?;

            metadata.insert("latest", header.serialize().as_slice())?;

            // Chain must have checked if block index immediately follows the
            // latest block
            let mut indexes = write_txn.open_table(INDEXES)?;

            indexes.insert(index, header.id().as_slice())?;

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
        if let Some(block) = blockinfo.block() {
            // Use block contracts to validate the state
            self.update_state_with_block(block)?;
        }

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
    chain: &'a Blockchain<B>,
}

impl<'a, B: Balances> Iterator for BlockChainIterator<'a, B> {
    type Item = BlockInfo;

    // Returns next block
    fn next(&mut self) -> Option<Self::Item> {
        let block_option = self
            .chain
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
    use marketplace_helpers::objects::WorkSize;
    use marketplace_primitives::{
        par_exec::{
            Bill, ExecuteTime, Tx, TxContract, TxIdentifier, TxOpt, TxOptRef, Whiteroom, WorkPtr,
        },
        sync_final::{BlockHeader, Proposal, Proposer, Validated, Validator},
    };
    use marketplace_wallet::Owner;
    use std::{
        assert_eq,
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::{mpsc, Arc, Barrier},
        thread,
        time::Duration,
        vec,
    };

    fn test_db(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "marketplace_ledger_{name}_{}_{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_file(&path);
        path
    }

    fn blockinfo(block: Block) -> BlockInfo {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, block);
        let proposal = Proposal::new(proposer, HashMap::new());
        let header = *proposal.block().header();
        let validator_owner = Owner::new_sig();
        let validator = Validator::new(&validator_owner, header);
        let mut validators = Whiteroom::<Validated, 33>::new();
        
        // A 33-member-capacity room expects 25 voters and requires 17 votes
        // for consensus, so this single weighted validator makes header() return Some.
        validators
            .add_member(Validated::new(validator, 17, HashMap::new()))
            .unwrap();

        BlockInfo::new(Some(proposal), validators)
    }

    // Test Transaction Contract where owner sends money to another and
    // the rest to himself
    fn ctr(owner: &Owner, owner1: &Owner) -> Contract {
        let ipt = Some(vec![TxOptRef::new([255; 32], 0)]);

        let opt = Some(vec![
            TxOpt::new(owner1.as_lock(), WU::try_from(30000).unwrap()),
            TxOpt::new(owner.as_lock(), WU::GDC() - WU::try_from(30000).unwrap()),
        ]);

        let bill = Bill::new(vec![Tx::new(
            0,
            TxIdentifier::COIN,
            ipt.clone(),
            opt.clone(),
        )]);

        let key = owner.sign(&bill.id());

        let mut txctr = TxContract::new(bill);

        txctr.add_auth(owner.as_lock(), key);

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
        assert_eq!(chain.len(), 1);

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

        assert!(chain.iter(chain.len()).next().is_none());
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

    // A cached contract can be looked up, removed exactly once, and reused by ID.
    #[test]
    fn cache_take_is_atomic_and_only_returns_existing_contract() -> StdResult<()> {
        let path = test_db("cache_take");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let contract = Contract::genesis();
        let id = contract.id();

        assert!(chain.take_ctr(id).is_none());
        chain.add_ctrs(vec![contract.clone()]);
        assert_eq!(chain.take_ctr(id).unwrap().id(), contract.id());
        assert!(chain.take_ctr(id).is_none());
        assert!(chain.cache().is_empty());

        fs::remove_file(path)?;
        Ok(())
    }

    // The same in-memory chain and database should survive reopening.
    #[test]
    fn published_block_persists_across_database_reopen() -> StdResult<()> {
        let path = test_db("persistence");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let block = chain.new_block();
        let block_id = block.id();
        chain.add_to_db(blockinfo(block))?;

        assert_eq!(chain.len(), 2);
        let latest = chain.latest_header()?;
        assert!(chain.is_block(&latest)?);

        drop(chain);
        let reopened = Blockchain::init(&path, State::<state::DefaultBalances>::new())?;

        assert_eq!(reopened.len(), 2);
        assert_eq!(reopened.latest_header()?.id(), block_id);
        assert_eq!(
            reopened
                .get_blockinfo_by_index(1)?
                .unwrap()
                .block()
                .unwrap()
                .id(),
            block_id,
        );

        fs::remove_file(path)?;
        Ok(())
    }

    // A confirmed block should remove its contracts from the cache and apply
    // their transactions to the state. This method does not confirm the block;
    // that is handled by publish_blockinfo before calling it.
    #[test]
    fn confirmed_block_consumes_cached_contract_and_updates_state() -> StdResult<()> {
        let path = test_db("confirmed_block");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let owner = Owner::new_sig();
        let recipient = Owner::new_sig();
        let contract = ctr(&owner, &recipient);
        let id = contract.id();
        chain.add_ctrs(vec![contract.clone()]);

        let block = Block::new(&chain.latest_header()?, BlockBody::new(vec![contract]));

        chain.update_state_with_block(&block)?;
        assert!(chain.take_ctr(id).is_none());

        fs::remove_file(path)?;
        Ok(())
    }

    // If a later transaction references a missing UTXO, UTXOs removed by earlier
    // transactions in the bill must be restored to the coin state.
    #[test]
    fn get_bill_utxos_rolls_back_earlier_transaction_on_missing_input() -> StdResult<()> {
        let path = test_db("bill_missing_input");
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let first_ref = TxOptRef::new([11; 32], 0);
        let missing_ref = TxOptRef::new([12; 32], 0);
        let first_output = TxOpt::new(lock, WU::try_from(100u128).unwrap());
        chain
            .state()
            .get_mut_coin_balance_list()
            .put(first_ref, first_output);

        let first = Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![first_ref]),
            Some(vec![TxOpt::new(lock, first_output.amount())]),
        );
        let second = Tx::new(
            2,
            TxIdentifier::COIN,
            Some(vec![missing_ref]),
            Some(vec![TxOpt::new(lock, WU::try_from(50u128).unwrap())]),
        );
        let bill = Bill::new(vec![first, second]);
        let mut auth = HashMap::new();
        auth.insert(lock.id(), owner.sign(&bill.id()));

        let err = chain.get_bill_utxos(&bill, &auth).unwrap_err();
        assert!(err.contains("could not be found"));
        assert_eq!(chain.gdc_balance(lock), first_output.amount());

        fs::remove_file(path)?;
        Ok(())
    }

    // Authentication failure in a later bill transaction must return the error
    // and restore UTXOs already removed for preceding transactions.
    #[test]
    fn get_bill_utxos_rolls_back_earlier_transaction_on_auth_failure() -> StdResult<()> {
        let path = test_db("bill_auth_failure");
        let owner = Owner::new_sig();
        let unauthenticated_owner = Owner::new_sig();
        let lock = owner.as_lock();
        let unauthenticated_lock = unauthenticated_owner.as_lock();
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let first_ref = TxOptRef::new([21; 32], 0);
        let second_ref = TxOptRef::new([22; 32], 0);
        let first_output = TxOpt::new(lock, WU::try_from(100u128).unwrap());
        let second_output = TxOpt::new(unauthenticated_lock, WU::try_from(200u128).unwrap());
        {
            let mut state = chain.state();
            let balances = state.get_mut_coin_balance_list();
            balances.put(first_ref, first_output);
            balances.put(second_ref, second_output);
        }

        let first = Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![first_ref]),
            Some(vec![TxOpt::new(lock, first_output.amount())]),
        );
        let second = Tx::new(
            2,
            TxIdentifier::COIN,
            Some(vec![second_ref]),
            Some(vec![TxOpt::new(lock, second_output.amount())]),
        );
        let bill = Bill::new(vec![first, second]);
        let mut auth = HashMap::new();
        auth.insert(lock.id(), owner.sign(&bill.id()));

        let err = chain.get_bill_utxos(&bill, &auth).unwrap_err();
        assert!(err.contains("Tx Output"));
        assert_eq!(chain.gdc_balance(lock), first_output.amount());
        assert_eq!(chain.gdc_balance(unauthenticated_lock), WU::default());

        fs::remove_file(path)?;
        Ok(())
    }

    // A bill whose transactions are all valid should remove every referenced
    // UTXO and retain no remaining native-coin balance.
    #[test]
    fn get_bill_utxos_removes_all_inputs_on_success() -> StdResult<()> {
        let path = test_db("bill_success");
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let refs = [TxOptRef::new([31; 32], 0), TxOptRef::new([32; 32], 0)];
        let amounts = [
            WU::try_from(100u128).unwrap(),
            WU::try_from(200u128).unwrap(),
        ];
        {
            let mut state = chain.state();
            let balances = state.get_mut_coin_balance_list();
            for (reference, amount) in refs.iter().zip(amounts.iter()) {
                balances.put(*reference, TxOpt::new(lock, *amount));
            }
        }

        let transactions = refs
            .iter()
            .zip(amounts.iter())
            .map(|(reference, amount)| {
                Tx::new(
                    1,
                    TxIdentifier::COIN,
                    Some(vec![*reference]),
                    Some(vec![TxOpt::new(lock, *amount)]),
                )
            })
            .collect();
        let bill = Bill::new(transactions);
        let mut auth = HashMap::new();
        auth.insert(lock.id(), owner.sign(&bill.id()));

        chain.get_bill_utxos(&bill, &auth)?;
        assert_eq!(chain.gdc_balance(lock), WU::default());

        fs::remove_file(path)?;
        Ok(())
    }

    // Authentication failure for a later work UTXO must restore UTXOs removed
    // before the failure and return the authentication error.
    #[test]
    fn get_workptr_utxos_rolls_back_on_later_auth_failure() -> StdResult<()> {
        let path = test_db("work_auth_failure");
        let owner = Owner::new_sig();
        let unauthenticated_owner = Owner::new_sig();
        let lock = owner.as_lock();
        let unauthenticated_lock = unauthenticated_owner.as_lock();
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let refs = [TxOptRef::new([41; 32], 0), TxOptRef::new([42; 32], 0)];
        let outputs = [
            TxOpt::new(lock, WU::try_from(10_000u128).unwrap()),
            TxOpt::new(unauthenticated_lock, WU::try_from(20_000u128).unwrap()),
        ];
        {
            let mut state = chain.state();
            let balances = state.get_mut_coin_balance_list();
            for (reference, output) in refs.iter().zip(outputs.iter()) {
                balances.put(*reference, *output);
            }
        }

        let work = WorkPtr::new(
            [51; 32],
            WorkSize::build(WU::try_from(15_000u128).unwrap()).unwrap(),
            refs.to_vec(),
            lock,
            BlockHeader::genesis(),
            ExecuteTime::NOW,
        );
        let mut auth = HashMap::new();
        auth.insert(lock.id(), owner.sign(&work.id()));

        let err = chain.get_workptr_utxos(&work, &auth).unwrap_err();
        assert!(err.contains("Tx Output"));
        assert_eq!(chain.gdc_balance(lock), outputs[0].amount());
        assert_eq!(chain.gdc_balance(unauthenticated_lock), WU::default());

        fs::remove_file(path)?;
        Ok(())
    }

    // Explicit job UTXO restoration must preserve every UTXO and its reference.
    #[test]
    fn revert_jobctr_utxos_restores_all_removed_utxos() -> StdResult<()> {
        let path = test_db("job_utxo_revert");
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let state: State<state::DefaultBalances> = State::new();
        let chain = Blockchain::init(&path, state)?;
        let refs = [TxOptRef::new([61; 32], 0), TxOptRef::new([62; 32], 1)];
        let outputs = [
            TxOpt::new(lock, WU::try_from(75u128).unwrap()),
            TxOpt::new(lock, WU::try_from(125u128).unwrap()),
        ];

        chain.revert_jobctr_utxos(refs.iter().copied().zip(outputs.iter().copied()).collect());
        assert_eq!(
            chain.gdc_balance(lock),
            outputs[0].amount() + outputs[1].amount(),
        );

        fs::remove_file(path)?;
        Ok(())
    }

    // Concurrent cache, state, database, and header operations must make
    // bounded progress and must not deadlock or poison any lock.
    #[test]
    fn concurrent_access_makes_progress_without_deadlock() -> StdResult<()> {
        let path = test_db("concurrency");
        let state: State<state::DefaultBalances> = State::new();
        let chain = Arc::new(Blockchain::init(&path, state)?);
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers + 1));
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();

        for worker in 0..workers {
            let chain = Arc::clone(&chain);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                for iteration in 0..100 {
                    let contract = Contract::genesis();
                    let id = contract.id();
                    chain.add_ctrs(vec![contract]);
                    let _ = chain.take_ctr(id);

                    let owner = Owner::new_sig();
                    let lock = owner.as_lock();
                    let _ = chain.gdc_balance(lock);
                    let _ = chain.latest_header();
                    let read = chain.db_read();
                    let _ = read.begin_read();
                    drop(read);

                    let _ = chain.get_blockinfo_by_index((worker + iteration) as u64 % 2);
                }
                sender.send(()).unwrap();
            }));
        }
        drop(sender);

        barrier.wait();
        for _ in 0..workers {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("concurrent blockchain operations deadlocked");
        }
        for handle in handles {
            handle
                .join()
                .expect("concurrent blockchain worker panicked");
        }

        fs::remove_file(path)?;
        Ok(())
    }
}
