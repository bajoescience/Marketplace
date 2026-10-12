use std::{collections::{HashMap, HashSet}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID}};
use marketplace_primitives::{Contract, JobContract, ResultPtr, TxContract};

/// # Jobs
/// 
/// The `Jobs` is the Live State of the marketplace where live jobs are stored and updated.
pub struct Jobs {
    // This is where jobs are kept
    // Each job contract is referenced by it's work pointer id
    jobs: HashMap<ID, JobContract>,

    // Store transaction contracts
    txs: Vec<TxContract>,

    // finalized jobs to be stored in mempool
    // pending block inclusion
    ready_jobs: HashSet<ID>,
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            txs: Vec::new(),
            ready_jobs: HashSet::new(),
        }
    }
}

// Getter methods
impl Jobs {
    /// Get the `JobContracts` that are ready to be finalized
    /// and added to a new `Block`
    pub fn get_pending_ctrs(&mut self) -> Vec<Contract> {
        let mut ctrs = Vec::new();

        // Get jobscontract from Hashmap
        // using it's id in ready_jobs list
        for ctr_id in self.ready_jobs.drain() {
            let jobctr = self.jobs.remove(&ctr_id)
                .expect("Illegal: Jobcontract not found in jobs yet it exists as finalized");


            ctrs.push(
                Contract::JOB(jobctr)
            )
        }

        for txctr in self.txs.drain(..) {
            ctrs.push(
                Contract::TX(txctr)
            );
        }

        ctrs
    }

    /// Take ownership of a `Job` from the `Jobs`
    pub fn take_job(&mut self, ctr_id: ID) -> Option<JobContract> {
        self.jobs.remove(&ctr_id)
    }
}

// Setter methods
impl Jobs {
    /// Add a new `Contract` to `Jobs`
    /// 
    /// **Note:**
    /// 
    /// Every `Transaction Contract` is automatically
    /// ready to be finalized.
    /// 
    /// The same is not true for a `Job Contract`.
    pub fn add_contract(&mut self, ctr: Contract) {
        match ctr {
            Contract::JOB(jobctr) => {
                let id = jobctr.work_id();

                self.jobs.insert(id, jobctr);
            },
            Contract::TX(txctr) => {
                self.txs.push(txctr);
            }
        }
    }

    /// Add a `Result Pointer` to `Job Contract` using it's id, 
    /// and return a boolean indicating if job consensus is possible
    /// 
    /// The caller function should handle the bool accordingly.
    pub fn add_resptr(&mut self, resptr: ResultPtr) -> AgentResult<bool> {
        let work_id = resptr.work_id();

        // Check if contract exists
        let Some(jobctr) = self.jobs.get_mut(&work_id) else {
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
            self.ready_jobs.insert(jobctr.work_id());
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add a contract to Jobs
    // both Job and Tx contracts

    // TODO: Update Job contract with Result Pointer

}