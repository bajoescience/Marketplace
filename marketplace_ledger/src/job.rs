use dashmap::{
    DashMap,
    mapref::one::{Ref, RefMut},
};
use std::{collections::HashSet, format, sync::Mutex};

use marketplace_helpers::{
    functions,
    objects::{AgentResult, ID},
};
use marketplace_primitives::par_exec::{Contract, JobContract, TxOpt, TxOptRef};
/// # Jobs
///
/// The `Jobs` is the Live State of the marketplace where live jobs are stored and updated.
pub struct Jobs {
    // This is where jobs are kept
    // Each job contract is referenced by it's work pointer id
    jobs: DashMap<ID, (JobContract, Vec<(TxOptRef, TxOpt)>)>,

    // finalized jobs to be stored in mempool
    // pending block inclusion
    ready_jobs: Mutex<HashSet<ID>>,
}

impl Jobs {
    pub fn new() -> Self {
        Self {
            jobs: DashMap::new(),
            ready_jobs: Mutex::new(HashSet::new()),
        }
    }
}

// Getter methods
impl Jobs {
    /// Get the `JobContracts` that are ready to be finalized
    /// and added to a new `Block`
    pub fn get_pending_jobs(&self) -> Vec<Contract> {
        let mut ctrs = Vec::new();

        // Get jobscontract from Hashmap
        // using it's id in ready_jobs list
        for ctr_id in self.ready_jobs.lock().unwrap().drain() {
            // Collect job contracts and drop utxos given that
            // the job has been finalized
            let (_, (jobctr, _)) = self
                .jobs
                .remove(&ctr_id)
                .expect("Illegal: Jobcontract not found in jobs yet it exists as finalized");

            ctrs.push(Contract::JOB(jobctr))
        }

        ctrs
    }

    /// Take ownership of a `Job` from the `Jobs`
    pub fn take_job(&self, id: &ID) -> Option<(JobContract, Vec<(TxOptRef, TxOpt)>)> {
        let (_, (jobctr, utxos)) = self.jobs.remove(id)?;

        // Remove the jobctr id if it was in the ready contract set
        self.ready_jobs.lock().unwrap().remove(id);

        Some((jobctr, utxos))
    }

    /// Get jobctr ref from jobs
    pub fn get_job(
        &self,
        id: &ID,
    ) -> AgentResult<Ref<'_, ID, (JobContract, Vec<(TxOptRef, TxOpt)>)>> {
        // Check if contract exists
        let Some(refs) = self.jobs.get(id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(id)
            ));
        };

        Ok(refs)
    }

    pub fn get_mut_job(
        &self,
        id: &ID,
    ) -> AgentResult<RefMut<'_, ID, (JobContract, Vec<(TxOptRef, TxOpt)>)>> {
        // Check if contract exists
        let Some(refs) = self.jobs.get_mut(id) else {
            return Err(format!(
                "Error: Job with work id {} does not exist",
                functions::from_bytes(id)
            ));
        };

        Ok(refs)
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
    pub fn add_contract(&self, jobctr: JobContract, utxos: Vec<(TxOptRef, TxOpt)>) {
        let id = jobctr.id();

        self.jobs.insert(id, (jobctr, utxos));
    }

    pub fn handle_consensus(&self, id: ID) {
        self.ready_jobs.lock().unwrap().insert(id);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier, mpsc},
        thread,
        time::Duration,
    };

    use marketplace_helpers::objects::{AgentResult, WU, WorkSize};
    use marketplace_primitives::{
        par_exec::{Contract, ExecuteTime, JobContract, TxOpt, TxOptRef, WorkPtr},
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
            header,
            ExecuteTime::NOW,
        );

        JobContract::new(work)
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
        let jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());

        assert_eq!(jobs.get_job(&id)?.value().0.id(), id);
        assert_eq!(jobs.get_mut_job(&id)?.value().0.id(), id);
        Ok(())
    }

    // A consensus-ready job should be emitted once and removed from the live job store.
    #[test]
    fn pending_jobs_are_drained_and_removed() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 6);
        let id = job.id();
        let jobs = Jobs::new();

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
        let jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());
        jobs.handle_consensus(id);

        assert!(jobs.take_job(&id).is_some());
        assert!(jobs.get_pending_jobs().is_empty());
    }

    // Taking a non-ready job should remove it and preserve its associated UTXOs.
    #[test]
    fn take_job_preserves_utxos_and_returns_one_owner_only() {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 8);
        let id = job.id();
        let jobs = Jobs::new();
        let reference = TxOptRef::new([9; 32], 0);
        let output = TxOpt::new(owner.as_lock(), WU::try_from(20_000).unwrap());
        let utxos = vec![(reference, output)];

        jobs.add_contract(job, utxos.clone());

        let (_, taken_utxos) = jobs.take_job(&id).expect("job should exist");
        assert_eq!(taken_utxos, utxos);
        assert!(jobs.take_job(&id).is_none());
    }

    // Replacing a live job with the same work ID should replace the contract and UTXOs.
    #[test]
    fn adding_the_same_job_id_replaces_live_entry() {
        let owner = Owner::new_sig();
        let first = job_contract(&owner, 10);
        let id = first.id();
        let replacement = first.clone();
        let jobs = Jobs::new();
        let first_output = TxOpt::new(owner.as_lock(), WU::try_from(10_000).unwrap());
        let replacement_output = TxOpt::new(owner.as_lock(), WU::try_from(20_000).unwrap());
        let reference = TxOptRef::new([13; 32], 0);

        jobs.add_contract(first, vec![(reference, first_output)]);
        jobs.add_contract(replacement, vec![(reference, replacement_output)]);

        assert_eq!(jobs.get_job(&id).unwrap().value().0.id(), id);
        let (_, utxos) = jobs.take_job(&id).expect("replacement job should exist");
        assert_eq!(utxos[0].1.amount(), replacement_output.amount());
    }

    // Repeated consensus handling is idempotent because the ready set is a HashSet.
    #[test]
    fn repeated_consensus_is_emitted_once() {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 11);
        let id = job.id();
        let jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());
        jobs.handle_consensus(id);
        jobs.handle_consensus(id);

        assert_eq!(jobs.get_pending_jobs().len(), 1);
        assert!(jobs.get_pending_jobs().is_empty());
    }

    // Marking an unknown ID ready should not panic when pending jobs are drained.
    #[test]
    #[should_panic(expected = "Jobcontract not found")]
    fn handling_unknown_job_exposes_missing_contract_in_pending_collection() {
        let jobs = Jobs::new();
        jobs.handle_consensus([77; 32]);

        assert!(jobs.get_pending_jobs().is_empty());
    }

    // Mutable job access should expose exactly the live entry and release after dropping.
    #[test]
    fn mutable_job_access_releases_for_later_operations() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let job = job_contract(&owner, 12);
        let id = job.id();
        let jobs = Jobs::new();

        jobs.add_contract(job, Vec::new());
        {
            let live = jobs.get_mut_job(&id)?;
            assert_eq!(live.value().0.id(), id);
        }

        assert!(jobs.take_job(&id).is_some());
        Ok(())
    }

    // Concurrent add/get/take/consensus operations should make bounded progress
    // and must not deadlock on the ready-set mutex or DashMap.
    #[test]
    fn concurrent_job_operations_make_progress_without_deadlock() {
        let jobs = Arc::new(Jobs::new());
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers + 1));
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();

        for worker in 0..workers {
            let jobs = Arc::clone(&jobs);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                let owner = Owner::new_sig();
                barrier.wait();

                for iteration in 0..100 {
                    let address = (worker * 100 + iteration) as u8;
                    let job = job_contract(&owner, address);
                    let id = job.id();

                    jobs.add_contract(job, Vec::new());
                    let _ = jobs.get_job(&id);
                    let _ = jobs.get_mut_job(&id);
                    let _ = jobs.take_job(&id);
                }

                sender.send(()).unwrap();
            }));
        }
        drop(sender);

        barrier.wait();
        for _ in 0..workers {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("concurrent Jobs operations deadlocked");
        }
        for handle in handles {
            handle.join().expect("concurrent Jobs worker panicked");
        }

        assert!(jobs.get_pending_jobs().is_empty());
    }
}
