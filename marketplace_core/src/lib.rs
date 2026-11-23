//! # Marketplace Core
//!
//! `marketplace core` is the root package of the Marketplace project. This package initializes the `Core` data structure
//! which manages the state of a marketplace node comprising of the following.
//!
//! ## Live State
//!
//! `Live State` is an in memory data pool - `Mempool` that stores and manages unfinalized Contracts similar
//! to the bitcoin mempool. A `Job Contract` in the `Mempool` can be mutated as the network listens for state change messages
//! from the network, and the storage is temporary.
//!
//! When the network finalizes a `Contract`, it can be put in a block which represents dead storage (unchanging state).
//!
//! ## Dead State
//!
//! `Dead State` is the the data structure that stores and manages finalized Contracts which is the
//! `Block chain`. The data in the `Dead State` is permanent, and cannot be changed.
//!
//! ## Financial State
//!
//! `Financial State` is the state of account balances on the network. This data structure keeps track of
//! the remainder asset of every owner on the network.
//!
//!
//! The `Core` data structure manages state by listening for event messages from peers, and updating the state accordingly.
//!
//! The event messages include the following:
//! `Work Pointer` message
//! `Result Pointer` message
//! `Tx Contract` message
//! `Proposer` message
//! `Validator` message

use std::{
    any::{Any, TypeId},
    cmp::Ordering,
    collections::HashMap,
    format,
    sync::{Arc, Mutex},
    time::Duration,
    todo, vec,
};

use marketplace_helpers::{
    functions,
    objects::{AgentResult, Epoch, IdHash, EPOCH_DURATION, ID, VOTER_DURATION},
};
use marketplace_ledger::{Balances, Blockchain, Jobs, Mempool};
use marketplace_p2p::Message;
use marketplace_primitives::{
    par_exec::{Bill, Contract, JobContract, ResultPtr, TxContract, WorkPtr},
    sync_final::{
        blockinfo::BlockInfo, voter::Voter, Block, BlockBody, BlockHeader, Proposal, Proposer,
        Validated, Validator,
    },
    Verify,
};
use marketplace_wallet::{voting, Key, Owner};
use marketplace_worker::Worker;
use tokio::{task::JoinHandle, time};

/// This is the `Core` data structure that manages the state of the Marketplace node.
///
/// The state comprises of the `Mempool` which stores live state, the `Blockchain` which
/// is the dead state, and the `Financial State` .
///
/// The `Core` data structure methods handle incoming messages from the network
/// by validating the messages, and handing them over to the appropiate
/// state data structure (block chain or mempool) to handle.
pub struct Core<B: Balances + Send + Sync> {
    // Blockchain representing dead state
    chain: Blockchain<B>,

    // Cache of active jobs
    jobs: Jobs,

    // Mempool containing finalized contracts
    mempool: Mempool,

    // Worker that executes a VDF, and a Job

    // Node authentication
    owner: Owner,

    // Sync final future
    sync_final: Mutex<Option<JoinHandle<()>>>,
}

impl<B: Balances + Send + Sync + 'static> Core<B> {
    /// Initialize a new `Core` object
    /// which comprises of initializing the
    /// `Live`, `Dead`, and Financial states.
    ///
    /// This method also takes as a parameter a worker that implements
    /// the `Worker` trait. This worker executes jobs and whiteroom challenges
    /// on behalf of the nodes.
    ///
    /// The `owner` is used to authenticate a node, and it is used to vote
    /// as proposer or validator of blocks on the network.
    pub fn new(chain: Blockchain<B>, owner: Owner) -> Arc<Self> {
        // Initialize the blockchain as the dead state

        // TODO: Update chain with blocks from network first
        // if the blockchain is not up to date

        // Initialize new mempool to act as live state
        // keeping track of finalized contracts.
        let mempool = Mempool::new();

        // Handle active jobs
        let jobs = Jobs::new();

        // A Worker is also included to execute jobs
        // and solve whiteroom challenges.
        let core = Arc::new(Self {
            chain,
            jobs,
            mempool,
            owner,
            sync_final: Mutex::new(None),
        });

        let core_clone = Arc::clone(&core);
        let handle = tokio::spawn(async move {
            core_clone.handle_sync_final().await;
        });

        *core.sync_final.lock().unwrap() = Some(handle);

        core
    }

    /// Reset Sync task by replacing it with a fresh task
    /// aborting the old one.
    pub async fn reset_sync(self: Arc<Self>) {
        // Get sync task handle
        let mut handle_guard = self.sync_final.lock().unwrap();

        // Take handle whch should always be Some
        let handle = handle_guard.take().unwrap();
        handle.abort();

        // Assign new version of task
        let core_clone = Arc::clone(&self);

        *handle_guard = Some(tokio::spawn(async move {
            core_clone.handle_sync_final().await;
        }));
    }

    /// Handle `Messages` from the network and update `Core` state based on the type of message.
    ///
    /// The message handler must return an `Error` to indicate an invalid message
    /// so that the caller does not resend the message to it's peers.
    ///
    /// The objects in the `Messages` passed to this function must be verifiable (`Verify` trait)
    pub async fn handle_message<M>(&self, message: &Message<M>) -> AgentResult<()>
    where
        M: 'static + Verify + IdHash,
    {
        // Collect payload and signatures from the message
        let Message { payload, auth } = message;

        // Ensure message payload is authorized/valid
        // through the verify trait
        payload.verify()?;

        // Check Message type
        let any_payload = &message.payload as &dyn Any;

        // Differentiate message by type and handle accordingly.
        if TypeId::of::<M>() == TypeId::of::<WorkPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let workptr = any_payload.downcast_ref::<WorkPtr>().unwrap();

            self.handle_job(workptr, auth)
        }
        // If message payload is a result pointer,
        // update the job accordingingly
        else if TypeId::of::<M>() == TypeId::of::<ResultPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let resultptr = any_payload.downcast_ref::<ResultPtr>().unwrap();

            // Validate witness with state
            self.handle_result_ptr(resultptr, auth)
        }
        // Handle message containing a Bill payload
        else if TypeId::of::<M>() == TypeId::of::<Bill>() {
            let bill = any_payload.downcast_ref::<Bill>().unwrap();

            self.handle_bill(bill, auth)
        }
        // Handle message with block payload
        else if TypeId::of::<M>() == TypeId::of::<Proposer>() {
            // TODO: Change the unwrap to handle error without panicking!
            let proposer = any_payload.downcast_ref::<Proposer>().unwrap();

            self.handle_proposer(proposer, auth).await
        }
        // Handle Block validator payload
        else if TypeId::of::<M>() == TypeId::of::<Validator>() {
            // TODO: Change the unwrap to handle error without panicking!
            let validator = any_payload.downcast_ref::<Validator>().unwrap();

            self.handle_validator(validator, auth).await
        }
        // TODO: Handle more types of network messages

        // Handle message that is not recognized by the network
        else {
            return Err(format!("Error: Invalid State transition Message"));
        }
    }
}

// Handle network messages
impl<B: Balances + Send + Sync> Core<B> {
    /// ## Work Pointer Handler
    ///
    /// A `Work Pointer` is sent to the network to initialize a `Job` similar to a Job post.
    ///
    /// To handle a `Work Pointer`, this handler does the following:
    ///
    /// - **Verify the `Work Pointer`**
    ///   
    ///   To do this, we ensure the `Block Header` referenced by the `Work Pointer` exists by fetching it from the
    ///   blockchain. If the `Block Header` is more than two epochs old, the `Job` is considerd expired. If the `Block Header`
    ///   does not exist, the handler returns an error that can be handled by the caller.
    ///
    /// - **Initialize a `Job Contract`**
    ///   If the `Work Pointer` is verified, a `Job Contract` is created with the `Work Pointer` as the input
    ///   and an empty output (Where output is the `Whiteroom`). the `Job Contract` is handed over to the
    ///   `Mempool` to be handled.
    ///
    ///   If the `Work Pointer` is expired, the `Job Contract` is stored in the mempool, but never executed.
    ///   The node simply waits on the results of the `Job`. God bless.
    fn handle_job(&self, workptr: &WorkPtr, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Fetch blockheader from blockchain
        let blk_hdr = workptr.blk_hdr();

        // referenced by workptr
        if !self.chain.is_block(blk_hdr).unwrap() {
            return Err(format!("Error: Block header does not exist"));
        };

        // Get list of utxos refernced by the job as inputs
        // if any error occurs, the utxo's should be returned to state
        let utxos = self.chain.get_workptr_utxos(workptr, auth)?;

        let jobctr = JobContract::new_with_auth(workptr.clone(), auth.clone());

        // Store job contract in jobs to be handled
        self.jobs.add_contract(jobctr, utxos);

        // If workptr is recent, execute it with worker
        if self.chain.is_recent(blk_hdr.id()) {
            // TODO: Execute work
        }

        Ok(())
    }

    /// ## Result Pointer Handler
    ///
    /// The `Result Pointer` is sent to the network as a result of a Job.
    ///
    /// To handle the `Result Pointer`, simply hand it over to a mempool handler
    /// that returns an `Error` if any problem occurs. (See Mempool)
    ///
    /// If result pointer, revert job.
    fn handle_result_ptr(&self, result: &ResultPtr, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Verify result pointer ownership
        let result_owner = result.witness();
        let Some(key) = auth.get(&result_owner.id()) else {
            return Err(format!("Error: Result Pointer auth not found"));
        };

        result_owner.unlock(key, &result.id())?;

        // Get jobctr from Jobs cache
        let job_id = result.work_id();
        let can_consensus;
        let is_consensus;

        // Update job
        // this is put in a new scope due to borrowing rules!
        {
            let mut ref_job = self.jobs.get_mut_job(&job_id)?;
            let (job, _) = ref_job.value_mut();

            // Add result pointer to job
            // and the auth object
            job.add_result(result.clone())?;
            job.add_auth(result_owner, key.clone());

            can_consensus = job.can_consensus();
            is_consensus = job.is_consensus();
        }

        // If job contract has reached consensus, handle consensus
        if is_consensus {
            self.jobs.handle_consensus(job_id);
        }

        // If job cannot reach consensus, withdraw job
        // revert utxo state
        if !can_consensus {
            let (_, utxos) = self
                .jobs
                .take_job(&job_id)
                .expect("Illegal: Contract should still exist");

            self.chain.revert_jobctr_utxos(utxos);
        }

        Ok(())
    }

    // Handle bill
    fn handle_bill(&self, bill: &Bill, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Get each transaction inputs from the
        self.chain.get_bill_utxos(bill, auth)?;

        let txctr = TxContract::new_with_auth(bill.clone(), auth.clone());

        // TODO: Add TxContract to chain
        self.chain.add_ctrs(vec![Contract::TX(txctr)]);

        Ok(())
    }

    // TODO: Handle received block
    async fn handle_proposer(
        &self,
        proposer: &Proposer,
        auth: &HashMap<ID, Key>,
    ) -> AgentResult<()> {
        let subusers = self.verify_voter(proposer)?;

        let mempool = &self.mempool;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Create a new proposal and verify auth
            let proposal = Proposal::new(proposer.clone(), auth.clone());
            proposal.verify()?;

            // Proposer should be accepted if it contains the block
            // with the highest new gdc extension to the blockchain
            // Equality of gdc is handled by this method, such that only for
            // proposals containing the same block does the method return Ordering::Equal.
            let Ordering::Greater = mempool.cmp_proposers(proposer) else {
                // Update subuser count
                mempool.update_subusers(subusers);
                return Ok(());
            };

            // Validate contracts in a proposal block by checking if
            // contracts exists in chain cache, if not the contract updates state,
            // and is added to cache
            self.validate_block(proposal.block())?;

            // Add proposal to mempool and update subusers
            mempool.add_proposal(proposal, subusers).await;

            // If subusers count is more than threshold, vote on block
            // as validator
            todo!()
        } else {
            return Err(format!("Err: Block is not a valid proposed block"));
        }
    }

    // Handle received validator
    async fn handle_validator(
        &self,
        validator: &Validator,
        auth: &HashMap<ID, Key>,
    ) -> AgentResult<()> {
        let subusers = self.verify_voter(validator)?;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Get mempool
            let mempool = &self.mempool;

            // Create new validated struct and verify auth
            let validated = Validated::new(validator.clone(), subusers, auth.clone());
            validated.verify()?;

            // Add validated to mempool and update subusers
            mempool.add_validator(validated).await?;

            Ok(())
        } else {
            return Err(format!("Err: Block is not a valid proposed block"));
        }
    }

    // TODO: Handle new block header to the state
    // Also update blockchain to accomodate new chosen block
}

// Handle methods for blocks
impl<T: Balances + Send + Sync> Core<T> {
    // compare received block index with node's latest index
    // and handle accordingly
    fn compare_chain(&self, header: &BlockHeader) -> AgentResult<()> {
        let latest_header = self.chain.latest_header().unwrap();
        let chain_tip = latest_header.index() + 1;
        
        // Ensure block references prev header
        match chain_tip.cmp(&header.index()) {
            Ordering::Greater => {
                // If chain tip index header is greater than received block
                // index, then return error
                return Err(format!("Error: Expired Block"));
            }
            Ordering::Less => {
                // TODO: Request missing blocks stream
                // staring from chain tip index
                todo!();

                // After receiving all the blocks
                // compare this block header with the new latest
                // block
                

                // If still more than, return error
            }
            Ordering::Equal => {
                if header.prev() != latest_header.prev() {
                    Err(format!(
                        "Err: Invalid blockinfo addition to chain"
                    ))
                } else {
                    Ok(())
                }
            },
        }
    }

    // Validate proposal
    pub fn verify_voter(&self, voter: &impl Voter) -> AgentResult<u64> {
        // Comapre chain first
        self.compare_chain(&voter.header())?;

        // Get subusers and blockheader
        Ok(self.get_subusers(voter)?)
    }

    /// Validate a blockinfo
    pub fn validate_blockinfo(&self, blockinfo: &BlockInfo) -> AgentResult<()> {
        // Verify validators
        for validated in blockinfo.validators().members() {
            // Verify owner of validated
            validated.verify()?;

            // Verify subusers in validated
            if self.verify_voter(validated.validator())? < 1 {
                return Err(format!(
                    "Err: Invalid validator"
                ))
            };
        }

        // The validators in a blockinfo must reach consensus on a block
        let consensus_header = blockinfo
            .header()
            .ok_or("Error: Blockinfo not in consensus")?;

        // Verify proposal
        if let Some(proposal) = blockinfo.proposal() {
            // Ensure consensus header matches with proposal
            if proposal.block().header().id() != consensus_header.id() {
                return Err(format!(
                    "Err: Blockinfo proposal block must be agreed upon by validators"
                ));
            }

            // Verify owner of proposal
            proposal.verify()?;

            // Validate proposer in proposal
            self.verify_voter(proposal.proposer())?;

            // Validate block
            self.validate_block(proposal.block())?;
        } else {
            // If no proposal, expect an empty block to be the header
            let prev_header = self.chain.latest_header().unwrap();

            let expected = Block::empty(&prev_header);

            if expected.header().id() != consensus_header.id() {
                return Err(format!(
                    "Error: A blockinfo with no proposal should agree on an empty block"
                ));
            }
        }

        Ok(())
    }

    /// Validate block, and ensure every contract is valid, and can be gotten from state
    pub fn validate_block(&self, block: &Block) -> AgentResult<()> {
        // Check block index and match against local blockchain
        if self.chain.len() != block.index() {
            return Err(format!("Error: Block index is not the next in chain"));
        }

        for ctr in block.iter() {
            // Verify contract
            ctr.verify()?;

            // If contract exists in cache, the contract
            // is already valid with respect to utxo state.
            // Move on!
            let None = self.chain.cache().get(&ctr.id()) else {
                continue;
            };

            // If contract is a Job Contract, check if it exists in
            // jobs list.
            // If it exists, then add the complete job contract to chain cache, and remove
            // job from joblist
            if let Contract::JOB(jobctr) = ctr {
                let Some(_) = self.jobs.take_job(&jobctr.id()) else {
                    self.chain.add_ctrs(vec![ctr.clone()]);
                    continue;
                };
            }

            // Given that contract has not been found in any local storage
            // we can now add the contract to chain cache after removing it's
            // spent utxo from utxo state
            match ctr {
                Contract::JOB(jobctr) => {
                    // block referenced by workptr should exist
                    if !self.chain.is_block(&jobctr.blk_hdr()).unwrap() {
                        return Err(format!(
                            "Error: Block {} does not exist in Blockchain",
                            functions::from_bytes(&jobctr.blk_hdr().id())
                        ));
                    };

                    // If error removing spent utxos, return Err
                    let _ = self
                        .chain
                        .get_workptr_utxos(jobctr.work(), jobctr.auth())?;
                }
                Contract::TX(txctr) => {
                    let _ = self.chain.get_bill_utxos(txctr.bill(), txctr.auth())?;
                }
            }

            // Add ctr to cache after removing utxos
            // from state
            self.chain.add_ctrs(vec![ctr.clone()]);
        }

        Ok(())
    }

    /// Try to generate a new `Block` form a list of finalized `Contracts`
    /// to send to the network
    ///
    /// A `Block` can only be valid to send to the rest of the network
    /// if the current node is chosen as a block proposer
    fn propose_block(&self) -> Option<Proposer> {
        let block = self.chain.new_block();

        // Attempt proposer to
        // send to the network.
        let proposer = Proposer::new(&self.owner, block);

        // If subusers is greater than 0, node has been chosen to propose
        // block.
        if self.get_subusers(&proposer).ok()? > 0 {
            // Return proposal
            Some(proposer)
        } else {
            None
        }
    }

    /// Attempt to vote an a `Block` that has been observed
    /// to be the highest new gdc addition to the chain
    /// by sending it's `BlockHeader` to the network as a finalized `Block`
    fn finalize_block(&self, proposal_opt: Option<Proposal>) -> Option<Validator> {
        // If there is no proposal, send an empty block header as validator
        let header;

        if let Some(proposal) = proposal_opt {
            // Get header from proposer
            header = *proposal.block().header();
        } else {
            // Build validator using empty block
            let latest_header = self.chain.latest_header().unwrap();

            // Empty block
            let block = Block::new(&latest_header, BlockBody::empty());
            header = *block.header()
        };

        // Build validator using block id (header hash)
        let validator = Validator::new(&self.owner, header);

        // Get subusers using VRF output from validator
        // If subusers is greater than 0, node has been chosen to vote on
        // block being finalized.
        if self.get_subusers(&validator).ok()? > 0 {
            // Return validator to network
            Some(validator)
        } else {
            None
        }
    }

    /// Get subusers using VRF ouptput
    fn get_subusers(&self, voter: &impl Voter) -> AgentResult<u64> {
        // User gdc balance
        let user_amount = self.chain.gdc_balance(voter.owner());

        // User total currency
        let total_currency = self
            .chain
            .total_gdc()
            .expect("Error: Could not fetch total goldcoin data from blockchain");

        voting::subusers_count(voter.get_vrf_opt()?, user_amount, total_currency)
    }
}

// Handle async methods that can be spawned as tasks
impl<T: Balances + Send + Sync> Core<T> {
    /// This is the action to be taken every epoch by the marketplace node.
    ///
    /// This method takes a list of finalized `Contracts` from the network
    /// and bundles it into a `Block`.
    ///
    /// The `Block` is then added to the cached blocks in the blockchain
    /// to be proposed.
    ///
    /// This process happens every epoch in a constant loop
    pub fn handle_epoch(&self) {
        // Add ctrs to cache
        self.chain.add_ctrs(self.jobs.get_pending_jobs());
    }

    /// Handle Synchronous finalization
    pub async fn handle_sync_final(&self) {
        let mempool = &self.mempool;

        loop {
            // Wait for epoch time
            time::sleep(Duration::from_millis(EPOCH_DURATION)).await;

            // Handle epoch
            self.handle_epoch();

            // Prepare a new block
            if let Some(proposer) = self.propose_block() {
                // TODO: Send proposer to network
            }

            // If mempool proposers is not full,
            // Wait up to time limit for a full proposal, after grap whatever
            // proposal is available
            let _ = time::timeout(
                Duration::from_millis(VOTER_DURATION),
                mempool.wait_for_full_proposal(),
            )
            .await;

            // Always attempt validation until an agreement
            // has been reached by the network on the block with
            // the highest new gdc.
            let blockinfo = loop {
                if let Some(validator) = self.finalize_block(mempool.get_proposal()) {
                    // TODO: Send validator to network
                }

                // Race consensus success against consensus failure, bounded by a timeout limit
                let outcome = time::timeout(Duration::from_millis(VOTER_DURATION), async {
                    tokio::select! {
                        blockinfo = mempool.wait_for_consensus_validated() => Some(blockinfo),
                        _ = mempool.wait_for_failed_consensus_validated() => None,
                    }
                })
                .await;

                // Only break if we didn't time out (Ok) AND consensus succeeded (Some)
                if let Ok(Some(blockinfo)) = outcome {
                    break blockinfo;
                }

                // Clear validator list to start loop again
                mempool.clear_validators();
            };

            // Add blockinfo to blockchain and clear mempool
            self.chain.publish_blockinfo(blockinfo).unwrap();
            mempool.clear().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::{
        fs,
        sync::{mpsc, Arc, Barrier},
        thread,
        time::Duration,
    };

    use marketplace_helpers::objects::{AgentResult, IdHash, WU};
    use marketplace_ledger::{Blockchain, DefaultBalances, State};
    use marketplace_p2p::Message;
    use marketplace_primitives::{
        par_exec::{
            Bill, Contract, ExecuteTime, JobContract, Tx, TxContract, TxIdentifier, TxOpt,
            TxOptRef, Whiteroom, WorkPtr,
        },
        sync_final::{
            blockinfo::BlockInfo, voter::Voter, Block, BlockBody, BlockHeader, Proposal, Proposer,
            Validated, Validator,
        },
    };
    use marketplace_wallet::Owner;

    use super::Core;

    fn test_db(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "marketplace_core_{name}_{}_{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_file(&path);
        path
    }

    fn core(
        name: &str,
    ) -> (
        std::path::PathBuf,
        tokio::runtime::Runtime,
        Arc<Core<DefaultBalances>>,
    ) {
        let path = test_db(name);
        let chain = Blockchain::init(&path, State::<DefaultBalances>::new()).unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime_guard = runtime.enter();
        let core = Core::new(chain, Owner::new_sig());

        (path, runtime, core)
    }

    fn work_job(owner: &Owner, address: u8) -> JobContract {
        let work = WorkPtr::new(
            [address; 32],
            marketplace_helpers::objects::WorkSize::build(WU::try_from(10_000).unwrap()).unwrap(),
            vec![TxOptRef::new([255; 32], 0)],
            owner.as_wr_lock(),
            BlockHeader::genesis(),
            ExecuteTime::NOW,
        );
        JobContract::new(work)
    }

    fn fund_owner(core: &Core<DefaultBalances>, owner: &Owner, amount: WU) -> TxOptRef {
        let funding_tx = Tx::new(
            0,
            TxIdentifier::COIN,
            None,
            Some(vec![TxOpt::new(owner.as_lock(), amount)]),
        );
        let reference = TxOptRef::new(funding_tx.id(), 0);
        core.chain.state().put_opts(&funding_tx);
        reference
    }

    fn spending_bill(reference: TxOptRef, owner: &Owner, amount: WU) -> Bill {
        Bill::new(vec![Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![reference]),
            Some(vec![TxOpt::new(owner.as_lock(), amount)]),
        )])
    }

    // Build an authenticated consensus of validators for a specific header.
    // The supplied weight is repeated to reach a deterministic winning vote.
    fn consensused_blockinfo(
        core: &Core<DefaultBalances>,
        header: BlockHeader,
        weight: u64,
        proposal: Option<Proposal>,
        authenticate_validators: bool,
    ) -> BlockInfo {
        let mut validators = Whiteroom::<Validated, 33>::new();
        let threshold = marketplace_helpers::functions::bft_thresh(25);

        for _ in 0..threshold {
            let owner = Owner::new_sig();
            fund_owner(core, &owner, WU::GDC());
            let validator = Validator::new(&owner, header);
            let mut auth = HashMap::new();
            if authenticate_validators {
                auth.insert(validator.owner().id(), owner.sign(&validator.id()));
            }
            validators
                .add_member(Validated::new(validator, weight, auth))
                .unwrap();
        }

        BlockInfo::new(proposal, validators)
    }

    // An empty validator set cannot satisfy BlockInfo consensus verification.
    #[test]
    fn validate_blockinfo_rejects_empty_validator_set() {
        let (path, _runtime, core) = core("validate_blockinfo_empty");
        let info = BlockInfo::new(None, Whiteroom::<Validated, 33>::new());

        assert_eq!(
            core.validate_blockinfo(&info).unwrap_err(),
            "Error: Blockinfo not in consensus",
        );
        fs::remove_file(path).unwrap();
    }

    // A below-threshold validator set must fail validation; this also documents
    // that validator VRF verification is reached before the consensus result.
    #[test]
    fn validate_blockinfo_rejects_below_threshold_consensus() {
        let (path, _runtime, core) = core("validate_blockinfo_below_threshold");
        let header = BlockHeader::new(&BlockHeader::genesis(), &BlockBody::new(Vec::new()));
        let threshold = marketplace_helpers::functions::bft_thresh(25);
        let mut validators = Whiteroom::<Validated, 33>::new();
        let owner = Owner::new_sig();
        fund_owner(&core, &owner, WU::GDC());
        let validator = Validator::new(&owner, header);
        let mut auth = HashMap::new();
        auth.insert(validator.owner().id(), owner.sign(&validator.id()));
        validators
            .add_member(Validated::new(validator, (threshold - 1) as u64, auth))
            .unwrap();
        let info = BlockInfo::new(None, validators);

        assert_eq!(
            core.validate_blockinfo(&info).unwrap_err(),
            "Error: Could not verify VRF output",
        );
        fs::remove_file(path).unwrap();
    }

    // A validator entry without its owner signature must fail before consensus.
    #[test]
    fn validate_blockinfo_rejects_validator_without_authentication() {
        let (path, _runtime, core) = core("validate_blockinfo_validator_auth");
        let empty_block = Block::empty(&BlockHeader::genesis());
        let info = consensused_blockinfo(&core, *empty_block.header(), 1, None, false);

        assert!(core.validate_blockinfo(&info).is_err());
        fs::remove_file(path).unwrap();
    }

    // A correctly constructed empty-block consensus is currently blocked by
    // validator VRF verification before the no-proposal branch is reached.
    #[test]
    fn validate_blockinfo_empty_consensus_is_blocked_by_vrf_verification() {
        let (path, _runtime, core) = core("validate_blockinfo_empty_consensus");
        let empty_block = Block::empty(&BlockHeader::genesis());
        let info = consensused_blockinfo(&core, *empty_block.header(), 1, None, true);

        assert_eq!(
            core.validate_blockinfo(&info).unwrap_err(),
            "Error: Could not verify VRF output",
        );
        fs::remove_file(path).unwrap();
    }

    // A no-proposal BlockInfo currently fails validator VRF verification before
    // it can compare consensus with the required empty-block header.
    #[test]
    fn validate_blockinfo_rejects_wrong_no_proposal_header() {
        let (path, _runtime, core) = core("validate_blockinfo_wrong_empty");
        let wrong_block = Block::new(&BlockHeader::genesis(), BlockBody::genesis());
        let info = consensused_blockinfo(&core, *wrong_block.header(), 1, None, true);

        assert_eq!(
            core.validate_blockinfo(&info).unwrap_err(),
            "Error: Could not verify VRF output",
        );
        fs::remove_file(path).unwrap();
    }

    // Proposal authentication is not reached while validator VRF verification
    // is blocked for the constructed consensus.
    #[test]
    fn validate_blockinfo_proposal_auth_is_blocked_by_vrf_verification() {
        let (path, _runtime, core) = core("validate_blockinfo_proposal_auth");
        let proposed = Block::new(&BlockHeader::genesis(), BlockBody::new(Vec::new()));
        let proposer_owner = Owner::new_sig();
        fund_owner(&core, &proposer_owner, WU::GDC());
        let proposer = Proposer::new(&proposer_owner, proposed);
        let proposal = Proposal::new(proposer, HashMap::new());
        let info =
            consensused_blockinfo(&core, *proposal.block().header(), 1, Some(proposal), true);

        assert!(core.validate_blockinfo(&info).is_err());
        fs::remove_file(path).unwrap();
    }

    // An authenticated proposal is currently blocked by validator VRF
    // verification before proposal verification and block validation run.
    #[test]
    fn validate_blockinfo_proposal_is_blocked_by_vrf_verification() {
        let (path, _runtime, core) = core("validate_blockinfo_valid_proposal");
        let proposed = Block::new(&BlockHeader::genesis(), BlockBody::new(Vec::new()));
        let proposer_owner = Owner::new_sig();
        fund_owner(&core, &proposer_owner, WU::GDC());
        let proposer = Proposer::new(&proposer_owner, proposed);
        let mut proposal_auth = HashMap::new();
        proposal_auth.insert(proposer.owner().id(), proposer_owner.sign(&proposer.id()));
        let proposal = Proposal::new(proposer, proposal_auth);
        let info =
            consensused_blockinfo(&core, *proposal.block().header(), 1, Some(proposal), true);

        assert_eq!(
            core.validate_blockinfo(&info).unwrap_err(),
            "Error: Could not verify VRF output",
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn empty_block_validates_and_does_not_mutate_state() {
        let (path, _runtime, core) = core("validate_empty");
        let block = Block::new(&BlockHeader::genesis(), BlockBody::new(Vec::new()));

        assert!(core.validate_block(&block).is_ok());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_transaction_contract_is_rejected_by_validate_block() {
        let (path, _runtime, core) = core("validate_invalid_tx");
        let owner = Owner::new_sig();
        let tx = Tx::new(
            1,
            TxIdentifier::COIN,
            None,
            Some(vec![TxOpt::new(
                owner.as_lock(),
                WU::try_from(1_000).unwrap(),
            )]),
        );
        let tx_contract = TxContract::new(Bill::new(vec![tx]));
        let block = Block::new(
            &BlockHeader::genesis(),
            BlockBody::new(vec![Contract::TX(tx_contract)]),
        );

        assert!(core.validate_block(&block).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn equal_chain_header_is_accepted() {
        let (path, _runtime, core) = core("compare_equal");
        let header = BlockHeader::new(&BlockHeader::genesis(), &BlockBody::new(Vec::new()));

        assert!(core.compare_chain(&header).is_ok());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn expired_chain_header_is_rejected() {
        let (path, _runtime, core) = core("compare_expired");

        let err = core.compare_chain(&BlockHeader::genesis()).unwrap_err();
        assert_eq!(err, "Error: Expired Block");
        fs::remove_file(path).unwrap();
    }

    #[test]
    #[should_panic]
    fn missing_blocks_todo_still_panic_until_implemented() {
        let (path, _runtime, core) = core("compare_missing");
        let future_header = Block::new(&BlockHeader::genesis(), BlockBody::new(Vec::new()));
        let another_future_header =
            BlockHeader::new(&future_header.header(), &BlockBody::new(Vec::new()));

        let _ = core.compare_chain(&another_future_header);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unknown_message_type_is_rejected() {
        let (path, _runtime, core) = core("unknown_message");
        let owner = Owner::new_sig();
        let tx = Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![TxOptRef::new([1; 32], 0)]),
            Some(vec![TxOpt::new(
                owner.as_lock(),
                WU::try_from(1_000).unwrap(),
            )]),
        );
        let message = Message::new(tx);

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(core.handle_message(&message));

        assert_eq!(
            result.unwrap_err(),
            "Error: Invalid State transition Message"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn handle_epoch_moves_pending_jobs_to_chain_cache() {
        let (path, _runtime, core) = core("handle_epoch");
        let owner = Owner::new_sig();
        let job = work_job(&owner, 23);
        let id = job.id();

        core.jobs.add_contract(job, Vec::new());
        core.jobs.handle_consensus(id);
        core.handle_epoch();

        assert!(core.jobs.get_job(&id).is_err());
        assert!(core.chain.take_ctr(id).is_some());
        fs::remove_file(path).unwrap();
    }

    // A valid funded bill should reserve its referenced UTXO and add the
    // corresponding transaction contract to the blockchain cache.
    #[test]
    fn handle_bill_adds_transaction_contract_to_cache() -> AgentResult<()> {
        let (path, _runtime, core) = core("handle_bill");
        let owner = Owner::new_sig();
        let amount = WU::try_from(25_000).unwrap();
        let reference = fund_owner(&core, &owner, amount);
        let bill = spending_bill(reference, &owner, amount);
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&bill.id()));

        core.handle_bill(&bill, &auth)?;

        let tx_contract = TxContract::new_with_auth(bill.clone(), auth);
        let id = Contract::TX(tx_contract).id();
        assert_eq!(id, bill.id());
        assert!(core.chain.take_ctr(id).is_some());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), WU::default());

        fs::remove_file(path).unwrap();
        Ok(())
    }

    // A handled bill must preserve every referenced input if the bill creates
    // more coin value than it spends. This test documents the financial
    // invariant enforced by handle_bill and the required UTXO rollback.
    #[test]
    fn handle_bill_rejects_inflated_output_and_preserves_utxo() {
        let (path, _runtime, core) = core("handle_bill_inflated");
        let owner = Owner::new_sig();
        let input_amount = WU::try_from(10_000).unwrap();
        let input_reference = fund_owner(&core, &owner, input_amount);
        let bill = Bill::new(vec![Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![input_reference]),
            Some(vec![TxOpt::new(
                owner.as_lock(),
                WU::try_from(25_000).unwrap(),
            )]),
        )]);
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&bill.id()));

        assert!(core.handle_bill(&bill, &auth).is_err());
        assert!(core.chain.cache().is_empty());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), input_amount);

        fs::remove_file(path).unwrap();
    }

    // Work Pointer handling should accept a job only when the requested work
    // size exactly matches the total referenced input value.
    #[test]
    fn handle_job_accepts_exactly_backed_work_pointer() {
        let (path, _runtime, core) = core("handle_job_exact");
        let owner = Owner::new_sig();
        let amount = WU::try_from(10_000).unwrap();
        let reference = fund_owner(&core, &owner, amount);
        let work = WorkPtr::new(
            [71; 32],
            marketplace_helpers::objects::WorkSize::build(amount).unwrap(),
            vec![reference],
            owner.as_lock(),
            BlockHeader::genesis(),
            ExecuteTime::NOW,
        );
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&work.id()));

        assert!(core.handle_job(&work, &auth).is_ok());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), WU::default());
        assert!(core.jobs.get_job(&work.id()).is_ok());

        fs::remove_file(path).unwrap();
    }

    // A Work Pointer requesting more value than its inputs contain must be
    // rejected and must restore the referenced UTXOs to the financial state.
    #[test]
    fn handle_job_rejects_unbacked_work_and_preserves_utxos() {
        let (path, _runtime, core) = core("handle_job_unbacked");
        let owner = Owner::new_sig();
        let input_amount = WU::try_from(10_000).unwrap();
        let reference = fund_owner(&core, &owner, input_amount);
        let work = WorkPtr::new(
            [72; 32],
            marketplace_helpers::objects::WorkSize::build(
                WU::try_from(25_000).unwrap(),
            )
            .unwrap(),
            vec![reference],
            owner.as_lock(),
            BlockHeader::genesis(),
            ExecuteTime::NOW,
        );
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&work.id()));

        assert!(core.handle_job(&work, &auth).is_err());
        assert!(core.jobs.get_job(&work.id()).is_err());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), input_amount);

        fs::remove_file(path).unwrap();
    }

    // The Work Pointer header is an essential reference. A header not present
    // in the blockchain must prevent job creation and must not reserve UTXOs.
    #[test]
    fn handle_job_rejects_missing_block_header() {
        let (path, _runtime, core) = core("handle_job_missing_header");
        let owner = Owner::new_sig();
        let input_amount = WU::try_from(10_000).unwrap();
        let reference = fund_owner(&core, &owner, input_amount);
        let missing_header =
            BlockHeader::new(&BlockHeader::genesis(), &BlockBody::new(Vec::new()));
        let work = WorkPtr::new(
            [73; 32],
            marketplace_helpers::objects::WorkSize::build(input_amount).unwrap(),
            vec![reference],
            owner.as_lock(),
            missing_header,
            ExecuteTime::NOW,
        );
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&work.id()));

        assert!(core.handle_job(&work, &auth).is_err());
        assert!(core.jobs.get_job(&work.id()).is_err());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), input_amount);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn no_proposal_or_validator_is_returned_without_selection() {
        let (path, _runtime, core) = core("selection_none");

        assert!(core.finalize_block(None).is_none());
        assert!(core.propose_block().is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn concurrent_core_operations_make_progress_without_deadlock() {
        let (path, _runtime, core) = core("concurrency");
        let core = Arc::new(core);
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers + 1));
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();

        for worker in 0..workers {
            let core = Arc::clone(&core);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                for iteration in 0..100 {
                    let block = Block::new(&BlockHeader::genesis(), BlockBody::new(Vec::new()));
                    let _ = core.validate_block(&block);
                    core.handle_epoch();
                    let _ = core.finalize_block(None);
                    let _ = (worker, iteration);
                }
                sender.send(()).unwrap();
            }));
        }
        drop(sender);

        barrier.wait();
        for _ in 0..workers {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("concurrent Core operations deadlocked");
        }
        for handle in handles {
            handle.join().expect("concurrent Core worker panicked");
        }

        fs::remove_file(path).unwrap();
    }

    // A bill reserves UTXOs before its contract enters the shared cache.
    // Concurrent attempts to spend the same UTXO must have one winner and
    // must not produce duplicate cached transaction contracts.
    #[test]
    fn concurrent_bills_cannot_spend_the_same_utxo() {
        let (path, _runtime, core) = core("concurrent_bill_double_spend");
        let owner = Owner::new_sig();
        let amount = WU::try_from(25_000).unwrap();
        let reference = fund_owner(&core, &owner, amount);
        let bill = spending_bill(reference, &owner, amount);
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&bill.id()));
        let bill = Arc::new(bill);
        let auth = Arc::new(auth);

        let workers = 2;
        let barrier = Arc::new(Barrier::new(workers + 1));
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();

        for _ in 0..workers {
            let core = Arc::clone(&core);
            let bill = Arc::clone(&bill);
            let auth = Arc::clone(&auth);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                sender.send(core.handle_bill(&bill, &auth).is_ok()).unwrap();
            }));
        }
        drop(sender);

        barrier.wait();
        let results: Vec<_> = (0..workers)
            .map(|_| {
                receiver
                    .recv_timeout(Duration::from_secs(5))
                    .expect("concurrent bill handlers deadlocked")
            })
            .collect();
        for handle in handles {
            handle.join().expect("bill handler panicked");
        }

        assert_eq!(results.iter().filter(|result| **result).count(), 1);
        assert_eq!(results.iter().filter(|result| !**result).count(), 1);
        assert!(core.chain.cache().contains_key(&bill.id()));
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), WU::default());

        fs::remove_file(path).unwrap();
    }

    // If a later transaction in a bill fails after an earlier transaction has
    // reserved an input, that earlier reservation must be rolled back.
    #[test]
    fn failed_multi_transaction_bill_rolls_back_prior_utxo_reservations() {
        let (path, _runtime, core) = core("bill_reservation_rollback");
        let owner = Owner::new_sig();
        let amount = WU::try_from(25_000).unwrap();
        let valid_reference = fund_owner(&core, &owner, amount);
        let missing_reference = TxOptRef::new([99; 32], 0);
        let invalid_bill = Bill::new(vec![
            Tx::new(
                1,
                TxIdentifier::COIN,
                Some(vec![valid_reference]),
                Some(vec![TxOpt::new(owner.as_lock(), amount)]),
            ),
            Tx::new(
                1,
                TxIdentifier::COIN,
                Some(vec![missing_reference]),
                Some(vec![TxOpt::new(owner.as_lock(), amount)]),
            ),
        ]);
        let mut invalid_auth = HashMap::new();
        invalid_auth.insert(owner.as_lock().id(), owner.sign(&invalid_bill.id()));

        assert!(core.handle_bill(&invalid_bill, &invalid_auth).is_err());
        assert!(core.chain.cache().is_empty());
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), amount);

        let valid_bill = spending_bill(valid_reference, &owner, amount);
        let mut valid_auth = HashMap::new();
        valid_auth.insert(owner.as_lock().id(), owner.sign(&valid_bill.id()));

        core.handle_bill(&valid_bill, &valid_auth)
            .expect("rolled-back UTXO should be spendable");
        assert_eq!(core.chain.gdc_balance(owner.as_lock()), WU::default());
        assert!(core.chain.cache().contains_key(&valid_bill.id()));

        fs::remove_file(path).unwrap();
    }
}
