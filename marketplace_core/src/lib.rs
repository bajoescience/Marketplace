//! # Marketplace Core
//! 
//! `marketplace core` is the root package of the Marketplace project. This package initializes the `Core` data structure 
//! which manages the state of a marketplace node comprising of both `Live State` and `Dead State`.
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
//! The `Core` data structure manages state by listening for event messages from peers, and updating the state accordingly.
//! 
//! The event messages include the following:
//! `Work Pointer` message
//! `Result Pointer` message
//! `Tx Contract` message
//! `Block Header` message
//! `Block` message
use std::{any::{Any, TypeId}, format};

use marketplace_helpers::objects::{AgentResult};
use marketplace_ledger::{Blockchain, Mempool, State};
use marketplace_p2p::Message;
use marketplace_primitives::{Contract, JobContract, ResultPtr, Verify, WorkPtr};
use marketplace_worker::Worker;

/// This is the `Core` data structure that manages the state of the Marketplace node.
/// 
/// The state comprises of the `Mempool` which stores live state, and the `Blockchain` which
/// is the dead state.
/// 
/// The `Core` data structure methods handle incoming messages from the network
/// by validating the messages, and handing them over to the appropiate 
/// state data structure (block chain or mempool) to handle.
pub struct Core<T: State, W: Worker> {
    // Blockchain representing dead state
    chain: Blockchain<T>,

    // Mempool containing live state
    mempool: Mempool<T, W>,
}

impl<T: State, W: Worker> Core<T, W> {
    /// Initialize a new `Core` object
    /// which comprises of initializing the 
    /// `Live` and `Dead` state.
    /// 
    /// This method also takes as a parameter a worker that implements
    /// the `Worker` trait. This worker executes jobs and whiteroom challenges
    /// on behalf of the nodes.

    // TODO: Should the worker be inside the core? why not just use a channel
    // where the core can communicate with a worker on the outside, as there may be multiple workers
    // for multiple types of work (whiteroom challenge or job execution) or even multiple types of jobs.
    // I'll open a discussion
    pub fn new(state: T, worker: Option<W>) -> Self {
        // Initialize the blockchain as the dead state
        let chain = Blockchain::new(state);

        // TODO: Update chain with blocks from network first
        // if the blockchain is not up to date

        // Initialize new mempool to act as live state
        // Using past data from the blockchain
        // 
        // A Worker is also included to execute jobs
        // and solve whiteroom challenges.
        let mempool = Mempool::new(&chain, worker);

        Self {
            chain,
            mempool,
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
        if TypeId::of::<M>() == TypeId::of::<WorkPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let workptr = any_payload.downcast_ref::<WorkPtr>().unwrap();

            self.handle_work_ptr(workptr)
        }

        // If message payload is a result pointer, 
        // update the job accordingingly
        else if TypeId::of::<M>() == TypeId::of::<ResultPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let resultptr = any_payload.downcast_ref::<ResultPtr>().unwrap();

            // Validate witness with state
            self.handle_result_ptr(resultptr)
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
impl<T: State, W: Worker> Core<T, W> {
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
    fn handle_work_ptr(&mut self, workptr: &WorkPtr) -> AgentResult<()> {
        // Fetch blockheader from blockchain
        // referenced by workptr
        let Some(blk_hdr) = self.chain.find_hdr(workptr.blk_hdr()) else {
            return Err(format!(
                "Error: Block header does not exist"
            ))
        };
        
        // Initialize new Job contract
        let jobctr = JobContract::new(
            workptr.clone(), 
            blk_hdr.clone()
        );

        // Handover job contract to mempool to handle
        self.mempool.add_job(Contract::JOB(jobctr));

        Ok(())
    }


    /// ## Result Pointer Handler
    /// 
    /// The `Result Pointer` is sent to the network as a result of a Job.
    /// 
    /// To handle the `Result Pointer`, simply hands it over to a mempool handler
    /// that returns an `Error` if any problem occurs. (See Mempool) 
    fn handle_result_ptr(&mut self, witness: &ResultPtr) -> AgentResult<()> {
        self.mempool.add_resptr(witness.clone())
    }

    // TODO: Handle block and handover block to mempool
    // Handle block that has no job in state also. 

    // TODO: Handle new block header to the state
    // Also update blockchain to accomodate new chosen block
}