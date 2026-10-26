use std::{collections::{HashMap, HashSet}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID}};
use marketplace_primitives::{Contract, JobContract, tx::TxOpt};

/// # Jobs
/// 
/// The `Jobs` is the Live State of the marketplace where live jobs are stored and updated.
pub struct Jobs {
    // This is where jobs are kept
    // Each job contract is referenced by it's work pointer id
    jobs: HashMap<ID, (JobContract, Vec<TxOpt>)>,

    // finalized jobs to be stored in mempool
    // pending block inclusion
    ready_jobs: HashSet<ID>,
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
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
            // Collect job contracts and drop utxos given that
            // the job has been finalized
            let (jobctr, _) = self.jobs.remove(&ctr_id)
                .expect("Illegal: Jobcontract not found in jobs yet it exists as finalized");


            ctrs.push(Contract::JOB(jobctr))
        }

        ctrs
    }

    /// Take ownership of a `Job` from the `Jobs`
    pub fn take_job(&mut self, id: &ID) -> Option<(JobContract, Vec<TxOpt>)> {
        let (jobctr, utxos) = self.jobs.remove(id)?;

        // Remove the jobctr id if it was in the ready contract set
        self.ready_jobs.remove(id);

        Some((jobctr, utxos))
    }

    /// Get jobctr ref from jobs
    pub fn get_job(&self, id: &ID) -> AgentResult<&JobContract> {
        // Check if contract exists
        let Some((jobctr, _)) = self.jobs.get(id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(id)
            ))
        };

        Ok(jobctr)
    }

    pub fn get_mut_job(&mut self, id: &ID) -> AgentResult<&mut JobContract> {
        // Check if contract exists
        let Some((jobctr, _)) = self.jobs.get_mut(id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(id)
            ))
        };

        Ok(jobctr)
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
    pub fn add_contract(&mut self, jobctr: JobContract, utxos: Vec<TxOpt>) {
        let id = jobctr.work_id();

        self.jobs.insert(id, (jobctr, utxos));
    }

    pub fn handle_consensus(&mut self, id: ID) {
        self.ready_jobs.insert(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add a contract to Jobs
    // both Job and Tx contracts

    // TODO: Update Job contract with Result Pointer

}