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
use std::{any::{Any, TypeId}, format, todo};

use marketplace_helpers::objects::{AgentResult, Epoch, ID, IdHash};
use marketplace_ledger::{Balances, Blockchain, Jobs, Mempool, State};
use marketplace_p2p::Message;
use marketplace_primitives::{Contract, JobContract, Proposal, ResultPtr, Validated, Validator, Verify, sync_final::SubBlock};
use marketplace_wallet::{Owner, voting};
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
    chain: Blockchain,

    // Cache of active jobs
    jobs: Jobs,

    // Mempool containing finalized contracts
    mempool: Mempool,

    // Financial State of the network
    state: State<B>,

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
    pub fn new(state: State<B>, worker: Option<W>, chain: Blockchain, owner: Owner) -> Self {
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
            state,
            worker,
            owner,
        }
    }

    /// Handle `Messages` from the network and update `Core` state based on the type of message.
    /// 
    /// The message handler must return an `Error` to indicate an invalid message
    /// so that the caller does not resend the message to it's peers.
    /// 
    /// The objects in the `Messages` passed to this function must be verifiable (`Verify` trait)
    pub async fn handle_message<M>(&mut self, message: &Message<M>) -> AgentResult<()>
    where M: 'static + Verify
    {
        // Ensure message payload is authorized/valid
        // through the verify trait
        message.payload.verify()?;

        // Check Message type
        let any_payload = &message.payload as &dyn Any;

        // Differentiate message by type and handle accordingly.
        if TypeId::of::<M>() == TypeId::of::<JobContract>() {
            // TODO: Change the unwrap to handle error without panicking!
            let workptr = any_payload.downcast_ref::<JobContract>().unwrap();

            self.handle_job(workptr)
        }

        // If message payload is a result pointer, 
        // update the job accordingingly
        else if TypeId::of::<M>() == TypeId::of::<ResultPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let resultptr = any_payload.downcast_ref::<ResultPtr>().unwrap();

            // Validate witness with state
            self.handle_result_ptr(resultptr)
        }

        // Handle message with block payload
        else if TypeId::of::<M>() == TypeId::of::<Proposal>() {
            // TODO: Change the unwrap to handle error without panicking!
            let proposal = any_payload.downcast_ref::<Proposal>().unwrap();

            self.handle_proposal(proposal)
        }

        // Handle Block validator payload
        else if TypeId::of::<M>() == TypeId::of::<Validated>() {
            // TODO: Change the unwrap to handle error without panicking!
            let validated = any_payload.downcast_ref::<Validated>().unwrap();

            self.handle_validator(validated)
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
    fn handle_job(&mut self, jobctr: &JobContract) -> AgentResult<()> {
        // Fetch blockheader from blockchain
        let blk_hdr = jobctr.input().blk_hdr();

        // referenced by workptr
        let Some(_) = self.chain.get_blockinfo_by_id(blk_hdr).unwrap() else {
            return Err(format!(
                "Error: Block header does not exist"
            ))
        };

        // If workptr is recent, execute it with worker
        if self.chain.is_recent(*blk_hdr).unwrap() {
            // TODO: Execute work
        }

        let ctr = Contract::JOB(jobctr.clone());

        // Update asset state using contract
        self.try_sub_ctr(&ctr)?;

        // Handover job contract to mempool to handle
        self.jobs.add_contract(ctr);

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
    fn handle_result_ptr(&mut self, witness: &ResultPtr) -> AgentResult<()> {
        let can_consensus = self.jobs.add_resptr(witness.clone())?;

        // If job cannot reach consensus, withdraw job
        // revert state
        if !can_consensus {
            let jobctr = self.jobs.take_job(witness.work_id())
                .expect("Illegal: Contract should still exist");

            let ctr = Contract::JOB(jobctr);

            self.revert_sub_ctr(&ctr);
        }

        Ok(())
    }

    // TODO: Handle received block
    fn handle_proposal(&mut self, proposal: &Proposal) -> AgentResult<()> {
        // Get subusers and blockheader
        let subusers = self.get_subusers(proposal.get_vrf_opt()?)?;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Add block to chain and update subusers
            self.chain.add_proposal(proposal, subusers);

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
    fn handle_validator(&mut self, validated: &Validated) -> AgentResult<()> {
        // Get subusers and blockheader
        let subusers = self.get_subusers(validated.get_vrf_opt()?)?;

        // Subusers must be greater than 0
        if subusers > 0 {
            // Add block to chain and update subusers

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

    // Subtracts Contract transaction inputs from the global state
    // If inputs could not be found, a double spend error is assumed.
    pub fn try_sub_ctr(&mut self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::JOB(jobctr) => {
                self.state.try_sub_ipt(&jobctr.get_tx())
            },
            Contract::TX(txctr) => {
                // Store tx already used to update state
                // so that if any error occurs later, all state
                // changes can be safely reversed
                let mut txs = Vec::new();

                for tx in txctr.get_tx().iter() {
                    if let Err(e) = self.state.try_sub_ipt(tx) {
                        // Handle error by reverting previous state changes
                        for tx in txs.into_iter().rev() {
                            self.state.revert_sub_ipt(tx);
                        }

                        return Err(e)
                    }

                    txs.push(tx);
                }

                Ok(())
            },
        }
    }

    // Revert an updated contract state
    // which is done in the reverse way state was updated
    // representing a rollback.
    pub fn revert_sub_ctr(&mut self, ctr: &Contract) {
        match ctr {
            Contract::JOB(jobctr) => {
                self.state.revert_sub_ipt(&jobctr.get_tx());
            },
            Contract::TX(txctr) => {
                // Rollback in reverse
                for tx in txctr.get_tx().iter().rev() {
                    self.state.revert_sub_ipt(tx);
                }
            },
        }
    }
}

// Handle methods for blocks
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
    pub fn handle_epoch(&mut self, epoch: Epoch) {
        // Get finalized contracts
        let ctrs = self.jobs.get_pending_ctrs();

        // Add contracts to new sub block instance
        let sub_block = SubBlock::new(ctrs);

        // Add subblock to mempool
        self.mempool.add_sub_block(sub_block);
    }

    /// Try to generate a new `Block` form a list of finalized `Contracts`
    /// to send to the network
    /// 
    /// A `Block` can only be valid to send to the rest of the network
    /// if the current node is chosen as a block proposer
    pub fn propose_block(&mut self) -> AgentResult<()> {
        // Get block from blockchain cache

        // Attempt proposal to 
        // send to the network.
        let proposal = Proposal::new(&self.owner, block);
        
        // If subusers is greater than 0, node has been chosen to propose
        // block.
        if self.get_subusers(proposal.get_vrf_opt()?)? > 0  {
            // TODO: Send proposal to network
            todo!()
        };

        Ok(())
    }

    /// Attempt to vote an a `Block` that has been observed
    /// to be the highest new gdc addition to the chain
    /// by sending it's `BlockHeader` to the network as a finalized `Block`
    pub fn finalize_block(&mut self) -> AgentResult<()> {
        // Get finalized block header from chain
        let Some(header) = self.chain.get_block_to_finalize() else {
            return Err(format!(
                "Error: No block to finalize"
            ))
        };

        // Build validator using block id (header hash)
        let validator = Validator::new(&self.owner, &header.id());

        // Get subusers using VRF output from validator
        // If subusers is greater than 0, node has been chosen to vote on
        // block being finalized.
        let subusers = self.get_subusers(
            validator.verify(&header.id())?
        )?;

        if subusers > 0 {
            // Create validated to send to the network
            let validated = Validated::new(header, validator, subusers);

            // TODO: Send validated to network
            todo!()
        };

        todo!()
    }

    /// Get subusers using VRF ouptput
    pub fn get_subusers(&self, vrf_opt: ID) -> AgentResult<u64> {
        // User gdc balance
        let user_amount = self.state.get_gdc_balance(self.owner.as_lock());

        // User total currency
        let total_currency = self.chain
            .total_gdc()
            .expect("Error: Could not fetch total goldcoin data from blockchain");

        voting::subusers_count(
            vrf_opt, 
            user_amount, 
            total_currency
        )
    }
}