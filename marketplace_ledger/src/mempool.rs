use std::{collections::{HashMap, HashSet}, format, todo};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_primitives::{Block, BlockHeader, Contract, ResultPtr, Whiteroom};
use marketplace_worker::Worker;

use crate::{Blockchain, State};

pub struct Mempool<T: State, W: Worker> {
    // This is where jobs and contracts are kept
    // Each contract is referenced by it's work pointer id
    ctrs: HashMap<ID, Contract>,

    // finalized contracts id's
    final_ctrs: HashSet<ID>,

    // Block headers received by node
    // during an epoch
    // But only 3 blocks with max new gdc
    // or rank should be received
    recv_headers: Vec<BlockHeader>,

    // Whiteroom to finalize block
    whiteroom: Whiteroom<BlockHeader>,

    // State
    state: T,

    // Worker to execute work
    worker: Option<W>,

    // Latest header
    header: BlockHeader,
}

impl<T: State, W: Worker> Mempool<T, W> {
    pub fn new(chain: &Blockchain<T>, worker: Option<W>) -> Self {
        Self {
            ctrs: HashMap::new(),
            final_ctrs: HashSet::new(),
            recv_headers: Vec::new(),
            whiteroom: Whiteroom::new(),
            state: chain.state.clone(),
            worker,
            header: *chain.latest_hdr()
        }
    }
}

// Getter methods
impl<T: State, W: Worker> Mempool<T, W> {
    // Choose one block out of received blocks
    // and Return ID
    pub fn winner_block(&self) -> ID {
        todo!()
    }
}

// Setter methods
impl<T: State, W: Worker> Mempool<T, W> {
    // Add job contract to mempool
    pub fn add_job(&mut self, ctr: Contract) {
        let id = match &ctr {
            Contract::JOB(jobctr) => jobctr.work_id(),

            // Tx Contracts are always considered finalized
            // We add it to final contracts
            Contract::TX(txctr) => {
                let id = txctr.id();
                self.final_ctrs.insert(id);

                id
            },
        };

        self.ctrs.insert(id, ctr);
    }

    // Add result pointer to contract
    // Using it's id
    pub fn add_resptr(&mut self, resptr: ResultPtr) -> AgentResult<()> {
        let work_id = resptr.work_id();

        // Check if contract exists
        let Some(Contract::JOB(jobctr)) = self.ctrs.get_mut(&work_id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(&resptr.work_id())
            ))
        };

        // Add result and get whiteroom size
        jobctr.add_result(resptr)?;

        // If consensus
        // add contract to finalized
        if jobctr.output().is_consensus() {
            self.final_ctrs.insert(jobctr.work_id());
        }

        // If no possible consensus
        // throwaway contract.
        if !jobctr.output().can_consensus() {
            // Revert contract opt
            self.state.revert_ipt(&jobctr.get_tx());
            
            // Clear contract
            self.ctrs.remove(&work_id);
        }

        Ok(())

    }

    // Publish mempool contracts into a block
    pub fn publish(&mut self) -> (BlockHeader, Block) {
        // Initialize block body
        let mut ctrs = Vec::new();

        for ctr_id in self.final_ctrs.iter() {
            let ctr = self.ctrs.get(ctr_id)
                .expect("Illegal: Contract not found in ctrs yet it exists as finalized");


            ctrs.push(ctr.clone())
        }
        
        // Clear final_ctrs
        self.final_ctrs.clear();

        // New block
        let block = Block::new(ctrs);

        // Return block and it's header
        (
            BlockHeader::new(&block, &self.header),
            block
        )

        // TODO: Send this block to the network
        // If chosen.
    }

    // Add block received from network to mempool
    pub fn add_recv_block(&mut self, blk_hdr: BlockHeader) {
        self.recv_headers.push(blk_hdr);

        // TODO: check if block header is ranked lower than 
        // other headers

        // Receive block and check block
    }

    // TODO: Add received block ID to whiteroom
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add a contract to mempool
    // both Job and Tx contracts

    // TODO: Update Job contract with Result Pointer

}