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
//! `Block Header` message
//! `Block` message

mod epoch_state;
use std::{any::{Any, TypeId}, cmp::Ordering, collections::HashMap, format, todo, vec};

use marketplace_helpers::{functions, objects::{AgentResult, Epoch, ID, IdHash}};
use marketplace_ledger::{Balances, Blockchain, Jobs, Mempool, State, state};
use marketplace_p2p::Message;
use marketplace_primitives::{Verify, par_exec::{Bill, Contract, JobContract, ResultPtr, TxContract, TxOpt, TxOptRef, WorkPtr}, sync_final::{Block, Proposal, Proposer, Validated, Validator, voter::Voter}};
use marketplace_wallet::{Key, Owner, voting};
use marketplace_worker::Worker;

/// This is the `Core` data structure that manages the state of the Marketplace node.
/// 
/// The state comprises of the `Mempool` which stores live state, the `Blockchain` which
/// is the dead state, and the `Financial State` .
/// 
/// The `Core` data structure methods handle incoming messages from the network
/// by validating the messages, and handing them over to the appropiate 
/// state data structure (block chain or mempool) to handle.
pub struct Core<B: Balances, W: Worker> {
    // Blockchain representing dead state
    chain: Blockchain<B>,

    // Cache of active jobs
    jobs: Jobs,

    // Mempool containing finalized contracts
    mempool: Mempool,

    // Worker that executes a VDF, and a Job
    worker: Option<W>,

    // Node authentication
    owner: Owner,

    // Internal state for changing stages in epoch
    // from proposal to validator to the next epoch
    // epoch_state: Option<Box<dyn EpochState>>
}

impl<B: Balances, W: Worker> Core<B, W> {
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
    pub fn new(worker: Option<W>, chain: Blockchain<B>, owner: Owner) -> Self {
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
        Self {
            chain,
            jobs,
            mempool,
            worker,
            owner,
        }
    }

    /// Access Financial State of the network
    pub fn utxo_state(&mut self) -> &mut State<B> {
        self.chain.state()
    }

    /// Handle `Messages` from the network and update `Core` state based on the type of message.
    /// 
    /// The message handler must return an `Error` to indicate an invalid message
    /// so that the caller does not resend the message to it's peers.
    /// 
    /// The objects in the `Messages` passed to this function must be verifiable (`Verify` trait)
    pub async fn handle_message<M>(&mut self, message: &Message<M>) -> AgentResult<()>
    where M: 'static + Verify + IdHash
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

            self.handle_proposer(proposer, auth)
        }

        // Handle Block validator payload
        else if TypeId::of::<M>() == TypeId::of::<Validator>() {
            // TODO: Change the unwrap to handle error without panicking!
            let validator = any_payload.downcast_ref::<Validator>().unwrap();

            self.handle_validator(validator, auth)
        }

        // TODO: Handle more types of network messages

        // Handle message that is not recognized by the network
        else {
            return Err(format!(
                "Error: Invalid State transition Message"
            ));
        }
    }

}


// Handle network messages
impl<B: Balances, W: Worker> Core<B, W> {
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
    fn handle_job(&mut self, workptr: &WorkPtr, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Fetch blockheader from blockchain
        let blk_hdr = workptr.blk_hdr();

        // referenced by workptr
        let Some(block) = self.chain.get_block_by_id(blk_hdr).unwrap() else {
            return Err(format!(
                "Error: Block header does not exist"
            ))
        };

        // Get list of utxos refernced by the job as inputs
        // if any error occurs, the utxo's should be returned to state
        let utxos = self.get_workptr_utxos(workptr, auth)?;

        let jobctr = JobContract::new_with_auth(
            workptr.clone(),
            block.header().clone(),
            auth.clone()
        );

        // Store job contract in jobs to be handled
        self.jobs.add_contract(jobctr, utxos);

        // If workptr is recent, execute it with worker
        if self.chain.is_recent(*blk_hdr).unwrap() {
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
    fn handle_result_ptr(&mut self, result: &ResultPtr, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Verify result pointer ownership
        let result_owner = result.witness();
        let Some(key) = auth.get(&result_owner.id()) else {
            return Err(format!(
                "Error: Result Pointer auth not found"
            ))
        };

        result_owner.unlock(key, &result.id())?;

        // Get jobctr from Jobs cache
        let job_id = result.work_id();
        let can_consensus;
        let is_consensus;

        // Update job
        // this is put in a new scope due to borrowing rules!
        {
            let job = self.jobs.get_mut_job(&job_id)?;

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
            let (_, utxos) = self.jobs.take_job(&job_id)
                .expect("Illegal: Contract should still exist");

            self.revert_jobctr_utxos(utxos);
        }

        Ok(())
    }

    // Handle bill
    fn handle_bill(&mut self, bill: &Bill, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Get each transaction inputs from the 
        self.get_bill_utxos(bill, auth)?;

        let txctr = TxContract::new_with_auth(bill.clone(), auth.clone());
        
        // TODO: Add TxContract to chain
        self.chain.add_ctrs(
            vec![Contract::TX(txctr)]
        );
        
        Ok(())
    }

    // TODO: Handle received block
    fn handle_proposer(&mut self, proposer: &Proposer, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // Get subusers and blockheader
        let subusers = self.get_subusers(proposer)?;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Proposer should be accepted if it contains the block 
            // with the highest new gdc extension to the blockchain
            // Equality of gdc is handled by this method, such that only for
            // proposals containing the same block does the method return Ordering::Equal.
            let Ordering::Greater = self.mempool.cmp_proposers(proposer) else {
                // Update subuser count
                self.mempool.update_subusers(subusers);
                return Ok(())
            };

            // Create a new proposal
            let proposal = Proposal::new(proposer.clone(), auth.clone());

            // TODO: Validate contracts in a proposal block by checking if
            // contracts exists in chain cache, if not the contract updates state,
            // and is added to cache
            self.validate_block(proposal.block())?;


            // Add proposal to mempool and update subusers
            self.mempool.add_proposal(proposal, subusers);

            // If subusers count is more than threshold, vote on block
            // as validator
            todo!()
        } else {
            return Err(format!(
                "Err: Block is not a valid proposed block"
            ))
        }
    }

    // Handle received validator
    fn handle_validator(&mut self, validator: &Validator, auth: &HashMap<ID, Key>) -> AgentResult<()> {
        // TODO: Get subusers and blockheader
        let subusers = self.get_subusers(validator)?;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Create new validated struct
            let validated = Validated::new(validator.clone(), subusers, auth.clone());

            // Add validated to mempool and update subusers
            self.mempool.add_validator(validated)?;

            // If subusers count is more than threshold, vote on block
            // as validator
            todo!()
        } else {
            return Err(format!(
                "Err: Block is not a valid proposed block"
            ))
        }
    }

    // TODO: Handle new block header to the state
    // Also update blockchain to accomodate new chosen block
}


// Handle Ledger State changes
impl<T: Balances, W: Worker> Core<T, W> {
    // Try to take input UTXO's for jobcontracts
    pub fn get_workptr_utxos(
        &mut self, 
        workptr: &WorkPtr, 
        auth: &HashMap<ID, Key>
    ) -> AgentResult<Vec<(TxOptRef, TxOpt)>> {
        // Get goldcoin utxo list, which is the only currency that work can be done with
        let balances = self.utxo_state().get_mut_coin_balance_list();

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
        let balances = self.utxo_state().get_mut_coin_balance_list();

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
            match self.utxo_state().get_ref_opts(tx, &bill.id(), auth) {
                Ok(ref_opts) => {
                    // TODO: Check ipts owners consented
                    // Store tx opts
                    tx_ref_opts.push((tx, ref_opts));
                },
                Err(e) => {
                    // Revert state changes
                    for (tx, ref_opts) in tx_ref_opts.into_iter() {
                        self.utxo_state().revert_ref_opts(tx, ref_opts);
                    }
                    return Err(e)
                }
                
            };
        }

        Ok(())
    }
}

// Handle methods for blocks
impl<T: Balances, W: Worker> Core<T, W> {
    /// Validate block, and ensure every contract is valid, and can be gotten from state
    pub fn validate_block(&mut self, block: &Block) -> AgentResult<()> {
        for ctr in block.iter() {
            // Verify contract
            ctr.verify()?;

            // If contract exists in cache, the contract 
            // is already valid with respect to utxo state.
            // Move on!
            let None = self.chain.get_ctr(&ctr.id()) else {
                continue;
            };

            // If contract is a Job Contract, check if it exists in
            // jobs list.
            // If it exists, then add complete job contract to chain cache, and remove
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
                    // If error removing spent utxos, return Err
                    let _ = self.get_workptr_utxos(
                        jobctr.input(), 
                        jobctr.auth()
                    )?;
                },
                Contract::TX(txctr) => {
                    let _ = self.get_bill_utxos(
                        txctr.bill(), 
                        txctr.auth()
                    )?;
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
    pub fn propose_block(&mut self) -> AgentResult<()> {
        let block = self.chain.new_block();

        // Attempt proposer to 
        // send to the network.
        let proposer = Proposer::new(&self.owner, block);
        
        // If subusers is greater than 0, node has been chosen to propose
        // block.
        if self.get_subusers(&proposer)? > 0  {
            // TODO: Send proposal to network
            todo!()
        };

        Ok(())
    }

    /// Attempt to vote an a `Block` that has been observed
    /// to be the highest new gdc addition to the chain
    /// by sending it's `BlockHeader` to the network as a finalized `Block`
    pub fn finalize_block(&mut self) -> AgentResult<()> {
        // Get proposer from mempool to finalize
        let Some(proposal) = self.mempool.get_proposal_to_finalize() else {
            return Err(format!(
                "Error: No proposal to finalize"
            ))
        };

        // Build validator using block id (header hash)
        let validator = Validator::new(
            &self.owner, 
            &self.chain.seed(), 
            proposal.proposer()
        );

        // Get subusers using VRF output from validator
        // If subusers is greater than 0, node has been chosen to vote on
        // block being finalized.
        let subusers = self.get_subusers(&validator)?;

        if subusers > 0 {
            // TODO: Send validator to network
            todo!()
        };

        todo!()
    }

    /// Get subusers using VRF ouptput
    pub fn get_subusers(&self, voter: &impl Voter,) -> AgentResult<u64> {
        // User gdc balance
        let user_amount = self.utxo_state().get_gdc_balance(voter.owner());

        // User total currency
        let total_currency = self.chain
            .total_gdc()
            .expect("Error: Could not fetch total goldcoin data from blockchain");

        voting::subusers_count(
            voter.get_vrf_opt()?, 
            user_amount, 
            total_currency
        )
    }
}

// Handle async methods that can be spawned as tasks
impl<T: Balances, W: Worker> Core<T, W> {
    /// This is the action to be taken every epoch by the marketplace node.
    /// 
    /// This method takes a list of finalized `Contracts` from the network
    /// and bundles it into a `Block`.
    /// 
    /// The `Block` is then added to the cached blocks in the blockchain
    /// to be proposed.
    /// 
    /// This process happens every epoch in a constant loop
    pub fn handle_epoch(&mut self) {
        // Add ctrs to cache
        self.chain.add_ctrs(
            self.jobs.get_pending_jobs()
        );
    }

    /// Handle Synchronous finalization
    pub fn handle_sync_final(&mut self) {
        // 
    }
}