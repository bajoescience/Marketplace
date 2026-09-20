use std::{collections::{HashMap, HashSet}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash}};
use marketplace_primitives::{Contract, ResultPtr};

/// # Mempool
/// 
/// The `Mempool` is the Live State of the marketplace where live jobs and 
/// unfinalized transactions are stored.
///
/// The `Mempool` takes the Unspent Transaction  Output (UTXO) state from the blockchain,
pub struct Mempool {
    // This is where jobs and contracts are kept
    // Each contract is referenced by it's work pointer id
    ctrs: HashMap<ID, Contract>,

    // complete contracts id's
    // pending finalization
    ready_ctrs: HashSet<ID>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            ctrs: HashMap::new(),
            ready_ctrs: HashSet::new(),
        }
    }
}

// Getter methods
impl Mempool {
    /// Get the `Contracts` that are ready to be finalized
    /// and added to a new `Block`
    pub fn get_pending_ctrs(&mut self) -> Vec<Contract> {
        let mut ctrs = Vec::new();

        // Get contract form Hashmap
        // using contract id in ready_ctrs list
        for ctr_id in self.ready_ctrs.iter() {
            let ctr = self.ctrs.remove(ctr_id)
                .expect("Illegal: Contract not found in ctrs yet it exists as finalized");


            ctrs.push(ctr.clone())
        }
        
        // Clear ready_ctrs
        self.ready_ctrs.clear();

        ctrs
    }

    /// Take ownership of a `Job` from the `Mempool`
    pub fn take_job(&mut self, ctr_id: ID) -> Option<Contract> {
        self.ctrs.remove(&ctr_id)
    }
}

// Setter methods
impl Mempool {
    /// Add a new `Contract` to `Mempool`
    /// 
    /// **Note:**
    /// 
    /// Every `Transaction Contract` is automatically
    /// ready to be finalized.
    /// 
    /// The same is not true for a `Job Contract`.
    pub fn add_job(&mut self, ctr: Contract) {
        let id = match &ctr {
            Contract::JOB(jobctr) => jobctr.work_id(),

            // Tx Contracts are always considered finalized
            // We add it to final contracts
            Contract::TX(txctr) => {
                let id = txctr.id();
                self.ready_ctrs.insert(id);

                id
            },
        };

        self.ctrs.insert(id, ctr);
    }

    /// Add a `Result Pointer` to `Job Contract` using it's id, 
    /// and return a boolean indicating if job consensus is possible
    /// 
    /// The caller function should handle the bool accordingly.
    pub fn add_resptr(&mut self, resptr: ResultPtr) -> AgentResult<bool> {
        let work_id = resptr.work_id();

        // Check if contract exists
        let Some(Contract::JOB(jobctr)) = self.ctrs.get_mut(&work_id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(&resptr.work_id())
            ))
        };

        // Add result to the job contract
        jobctr.add_result(resptr)?;

        // If consensus
        // add contract to ready_contracts list
        if jobctr.output().is_consensus() {
            self.ready_ctrs.insert(jobctr.work_id());
        }

        // If no possible consensus
        // throwaway contract.
        if !jobctr.output().can_consensus() {
            // Caller function handles reverting
            // of asset state changes caused by the contract
            return Ok(false)
        }

        Ok(true)

    }

    /// Return `Contracts` to `Mempool`
    /// which where not finalized in a block.
    pub fn return_ctrs(&mut self, ctrs: Vec<Contract>) {
        for ctr in ctrs {
            // Get contract id
            let id = match &ctr {
                Contract::JOB(jobctr) => jobctr.work_id(),
    
                // Tx Contracts are always considered finalized
                // We add it to final contracts
                Contract::TX(txctr) => txctr.id()
            };

            // Add Contract to ready contracts
            self.ready_ctrs.insert(id);
            self.ctrs.insert(id, ctr); 
        }       
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add a contract to mempool
    // both Job and Tx contracts

    // TODO: Update Job contract with Result Pointer

}