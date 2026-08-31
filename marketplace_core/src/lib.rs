use std::{any::{Any, TypeId}, format};

use marketplace_helpers::objects::{AgentResult};
use marketplace_ledger::{Blockchain, Mempool, State};
use marketplace_p2p::Message;
use marketplace_primitives::{Contract, JobContract, ResultPtr, Verify, WorkPtr};
use marketplace_worker::Worker;

// Store jobs
pub struct Core<T: State, W: Worker> {
    // Blockchain representing dead state
    chain: Blockchain<T>,

    // Mempool containing live state
    mempool: Mempool<T, W>,
}

impl<T: State, W: Worker> Core<T, W> {
    // Create new mempool instance
    pub fn new(state: T, worker: Option<W>) -> Self {
        // New chain
        let chain = Blockchain::new(state);

        // TODO: Update chain with blocks from network

        // New mempool to act as live state
        let mempool = Mempool::new(&chain, worker);

        Self {
            chain,
            mempool,
        }
    }

    // handle Message
    // Return value must be of Result type
    // to indicate to caller not to resend message
    // messages passed to this function must be verifiable (Verify trait)
    pub async fn handle_message<M>(&mut self, message: &Message<M>) -> AgentResult<()>
    where M: 'static + Verify
    {
        // Ensure message payload is authorized/valid
        message.payload.verify()?;

        // Check message type
        let any_payload = &message.payload as &dyn Any;

        // Check if type matches a work pointer
        // and handle accordingly
        if TypeId::of::<M>() == TypeId::of::<WorkPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let workptr = any_payload.downcast_ref::<WorkPtr>().unwrap();

            self.handle_work_ptr(workptr)
        }

        // If message payload is a result pointer, update the job accordingingly
        else if TypeId::of::<M>() == TypeId::of::<ResultPtr>() {
            // TODO: Change the unwrap to handle error without panicking!
            let resultptr = any_payload.downcast_ref::<ResultPtr>().unwrap();

            // Validate witness with state
            self.handle_result_ptr(resultptr)
        }
        else {
            return Err(format!(
                "Error: State transition Message Not Valid"
            ));
        }
    }

}


// Handle Job message lifecycle
impl<T: State, W: Worker> Core<T, W> {
    // Create a new contract
    fn handle_work_ptr(&mut self, workptr: &WorkPtr) -> AgentResult<()> {
        // Fetch blockheader from blockchain
        // referenced by workptr
        let Some(blk_hdr) = self.chain.find_hdr(workptr.blk_hdr()) else {
            return Err(format!(
                "Error: Block header does not exist"
            ))
        };
        
        // Initialize new contract
        let jobctr = JobContract::new(
            workptr.clone(), 
            blk_hdr.clone()
        );

        // Handover contract to mempool
        // To store as a job
        self.mempool.add_job(Contract::JOB(jobctr));

        Ok(())
    }


    // Validate execution result and handover to job 
    fn handle_result_ptr(&mut self, witness: &ResultPtr) -> AgentResult<()> {
        self.mempool.add_resptr(witness.clone())
    }

    // Validate block and handover block to job
    // Handle block that has no job in state also. 

    // Add a new block header to the state
    // Also update blockchain to accomodate new chosen block
}