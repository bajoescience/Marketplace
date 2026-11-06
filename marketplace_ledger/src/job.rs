use std::{collections::{HashMap, HashSet}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID}};
use marketplace_primitives::par_exec::{Contract, JobContract, TxOpt, TxOptRef};

/// # Jobs
/// 
/// The `Jobs` is the Live State of the marketplace where live jobs are stored and updated.
pub struct Jobs {
    // This is where jobs are kept
    // Each job contract is referenced by it's work pointer id
    jobs: HashMap<ID, (JobContract, Vec<(TxOptRef, TxOpt)>)>,

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
    pub fn get_pending_jobs(&mut self) -> Vec<Contract> {
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
    pub fn take_job(&mut self, id: &ID) -> Option<(JobContract, Vec<(TxOptRef, TxOpt)>)> {
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
    pub fn add_contract(&mut self, jobctr: JobContract, utxos: Vec<(TxOptRef, TxOpt)>) {
        let id = jobctr.id();

        self.jobs.insert(id, (jobctr, utxos));
    }

    pub fn handle_consensus(&mut self, id: ID) {
        self.ready_jobs.insert(id);
    }
}

#[cfg(test)]
mod tests {
    use marketplace_helpers::objects::{IdHash, WU, WorkSize};
    use marketplace_helpers::objects::AgentResult;
    use marketplace_primitives::{
        par_exec::{Contract, ExecuteTime, JobContract, TxOptRef, WorkPtr},
        sync_final::BlockHeader,
    };
    use marketplace_wallet::Owner;

    use super::Jobs;

    fn job_contract(owner: &Owner, address: u8) -> JobContract {
        let header = BlockHeader::genesis();
        let work = WorkPtr::new(
            [address; 32],
            WorkSize::build(WU::try_from(10_000).unwrap()).unwrap(),
            vec![TxOptRef::new([255; 32], 0)],
            owner.as_wr_lock(),
            header.id(),
            ExecuteTime::NOW,
        );

        JobContract::new(work, header)
    }

    // Jobs should return an error when the requested work pointer is absent.
    #[test]
    fn missing_job_returns_an_error() {
        let jobs = Jobs::new();

        assert!(jobs.get_job(&[1; 32]).is_err());
    }

    // Adding a job should make it retrievable and allow mutable access to its contract.
    #[test]
    fn added_job_can_be_read_and_mutably_accessed() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 5);
        let id = job.id();
        let mut jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());

        assert_eq!(jobs.get_job(&id)?.id(), id);
        assert_eq!(jobs.get_mut_job(&id)?.id(), id);
        Ok(())
    }

    // A consensus-ready job should be emitted once and removed from the live job store.
    #[test]
    fn pending_jobs_are_drained_and_removed() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 6);
        let id = job.id();
        let mut jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());
        jobs.handle_consensus(id);

        let pending = jobs.get_pending_jobs();
        assert_eq!(pending.len(), 1);
        assert!(matches!(pending[0], Contract::JOB(_)));
        assert!(jobs.get_job(&id).is_err());
        assert!(jobs.get_pending_jobs().is_empty());
        Ok(())
    }

    // Taking a consensus-ready job should also clear it from the pending set.
    #[test]
    fn taking_ready_job_removes_it_from_pending_jobs() {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 7);
        let id = job.id();
        let mut jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());
        jobs.handle_consensus(id);

        assert!(jobs.take_job(&id).is_some());
        assert!(jobs.get_pending_jobs().is_empty());
    }
}