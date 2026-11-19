mod proposed;

pub use self::proposed::ProposedBlock;

pub use parking_lot::{Mutex, MutexGuard, MappedMutexGuard};

use std::{cmp::Ordering, todo};
use tokio::sync::{Notify, RwLock, RwLockReadGuard};

use marketplace_primitives::{
    par_exec::Whiteroom, sync_final::{Proposal, Proposer, Validated, blockinfo::BlockInfo},
};

use marketplace_helpers::{functions, objects::{AgentResult, IdHash}};
use marketplace_wallet::voting::EXPECTED_VOTERS;

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

/// Contains
struct SyncLists {
    // Handle proposal
    proposed: ProposedBlock, 

    // Validators
    validators: Whiteroom<Validated, WHITEROOM_MAX>,
}

impl SyncLists {
    fn new() -> Self {
        Self {
            proposed: ProposedBlock::new(),
            validators: Whiteroom::new(),
        }
    }

    fn clear_validators(&mut self) {
        let mut new_validators = Whiteroom::new();

        if let Some(proposal) = self.proposed.proposal() {
            // For each member, if the member has the same block as the proposal
            // add that member
            for member in self.validators.owned_members() {
                if member.validator().header().id() == proposal.block().id() {
                    new_validators.add_member(member).unwrap();
                }
            }
        }

        self.validators = new_validators;
    }
}

struct NotifyObject {
    // Notify object to notify when proposal is full
    proposer_notify: Notify,

    // Notify object to notify when validator has reach consensus
    validator_notify: Notify,

    // Notify when validator cannot reach consensus
    validator_not_notify: Notify,
}

impl NotifyObject {
    fn new() -> Self {
        Self {
            proposer_notify: Notify::new(),
            validator_notify: Notify::new(),
            validator_not_notify: Notify::new(),
        }
    }
}

pub struct Mempool {
    // Object to notify synclist copletion
    notify: RwLock<Option<NotifyObject>>,

    // Sync final lists
    sync_lists: Mutex<Option<SyncLists>>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            notify: RwLock::new(Some(NotifyObject::new())),
            sync_lists: Mutex::new(Some(SyncLists::new())),
        }
    }

    /// Lock sync lists to be able to access
    fn sync_lists(&self) -> MappedMutexGuard<'_, SyncLists> {
        MutexGuard::map(
            self.sync_lists.lock(),
            |list_opt| list_opt.as_mut().unwrap()
        )
    }

    /// Acces notify as read only
    async fn notify(&self) -> RwLockReadGuard<'_, NotifyObject> {
        let guard = self.notify.read().await;

        RwLockReadGuard::map(
            guard, 
            |notify_opt| notify_opt.as_ref().unwrap()
        )
    }

    /// Clear Mempool sync lists entries
    pub async fn clear(&self) {
        let mut list_opt = self.sync_lists.lock();
        let mut notify_opt = self.notify.write().await;

        // Clear objects
        let (_, _) = (list_opt.take(), notify_opt.take());

        // Assign new instances of objects
        *list_opt = Some(SyncLists::new());
        *notify_opt = Some(NotifyObject::new());
    }

    /// Clear only validator sync list
    pub fn clear_validators(&self) {
        self
        .sync_lists()
        .clear_validators();
    }
}

// Async methods 
// some check for full proposal or validator list
impl Mempool {
    /// If proposal list is full, get the proposal
    /// to be validated
    pub async fn wait_for_full_proposal(&self) {

        self.notify().await.proposer_notify.notified().await
    }

    /// If validated list has reached consensus, get blockinfo
    pub async fn wait_for_consensus_validated(&self) -> BlockInfo {
        self.notify().await.validator_notify.notified().await;

        self.get_blockinfo().await
    }

    /// If validated, do something real.
    pub async fn wait_for_failed_consensus_validated(&self) {
        self.notify().await.validator_not_notify.notified().await;
    }

    /// Get block info from a list of validators.
    /// If the mempool does not contain the chosen block
    /// fetch it from the network by request.
    pub async fn get_blockinfo(
        &self,
    ) -> BlockInfo {
        // Validators
        let validators = self.get_validators();

        // Get winning block id
        // Validators must be in consensus
        let block_id = *validators.winning_vote().unwrap().0;

        let sync_lists = self.sync_lists();
        let proposal = sync_lists.proposed.get_proposal().unwrap();

        // If self does not hold the proposal with a similar block
        // try to receive block
        if proposal.block().id() != block_id {
            // Request proposal
            todo!()
        }

        BlockInfo::new(
            proposal.clone(), 
            validators
        )
    }
}

// Setter methods
impl Mempool {
    /// Check if proposal is full
    pub fn is_full_proposals(&self) -> bool {
        self.sync_lists().proposed.is_full()
    }

    /// Add a proposal to the MempoolState and update total subusers count
    pub async fn add_proposal(&self, proposal: Proposal, subusers: u64) {
        let proposed = &mut self.sync_lists().proposed;

        proposed.add(proposal, subusers);

        // Check if proposal is full
        if proposed.is_full() {
            self.notify().await.proposer_notify.notify_one();
        }
    }

    // Update proposal count
    pub fn update_subusers(&self, subusers: u64) {
        self.sync_lists().proposed.update_subusers(subusers);
    }

    /// Get winning proposal to validate to network
    pub fn get_proposal(&self) -> Option<Proposal> {
        self.sync_lists().proposed.get_proposal().cloned()
    }

    /// Compare blocks to find which one is to be proposed
    pub fn cmp_proposers(&self, proposer: &Proposer) -> Ordering {
        self.sync_lists().proposed.cmp_block_gdc(proposer)
    }

    /// Check if validator whiteroom is full
    pub fn is_consensus_validators(&self) -> bool {
        self.sync_lists().validators.is_consensus()
    }

    /// Get block info to put in blockchain
    pub fn get_validators(&self) -> Whiteroom<Validated, 33> {
        self.sync_lists().validators.clone()
    }

    /// Add validator to mempool
    pub async fn add_validator(&self, validated: Validated) -> AgentResult<()> {
        let validators = &mut self.sync_lists().validators;

        validators.add_member(validated)?;

        // Check if validators has reached consensus
        if validators.is_consensus() {
            self.notify().await.validator_notify.notify_one();
        }

        // Check if validators can reach consensus
        if !validators.can_consensus() {
            self.notify().await.validator_not_notify.notify_one();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

    use marketplace_primitives::{
        par_exec::WRVote,
        sync_final::{
            Block, BlockBody, BlockHeader, Proposal, Proposer, Validated, Validator,
        },
    };
    use marketplace_wallet::Owner;
    use marketplace_wallet::voting::EXPECTED_VOTERS;
    use tokio::time::timeout;

    use super::Mempool;

    fn proposal() -> Proposal {
        let owner = Owner::new_sig();
        Proposal::new(Proposer::new(&owner, Block::genesis()), Default::default())
    }

    fn validated_for(proposer: &Proposer) -> Validated {
        let owner = Owner::new_sig();
        let seed = proposer.block().header().prev();
        let validator = Validator::new(&owner, &seed, proposer);

        Validated::new(validator, 1, HashMap::new())
    }

    // A new mempool should contain no proposal to finalize.
    #[test]
    fn new_mempool_has_no_proposal() {
        let mempool = Mempool::new();

        assert!(mempool.get_proposal().is_none());
    }

    // Adding a proposal should make it available for finalization.
    #[tokio::test]
    async fn added_proposal_can_be_retrieved() {
        let mempool = Mempool::new();

        mempool.add_proposal(proposal(), 2).await;

        assert!(mempool.get_proposal().is_some());
    }

    // Clearing the mempool should remove its current proposal and validator votes.
    #[tokio::test]
    async fn clear_removes_the_current_proposal() {
        let mempool = Mempool::new();
        mempool.add_proposal(proposal(), 2).await;

        mempool.clear().await;

        assert!(mempool.get_proposal().is_none());
        assert!(mempool.get_validators().members().next().is_none());
        assert!(!mempool.is_full_proposals());
    }

    // Proposal subusers should determine when proposal collection is full.
    #[tokio::test]
    async fn proposal_becomes_full_at_expected_voter_count() {
        let mempool = Mempool::new();

        mempool
            .add_proposal(proposal(), EXPECTED_VOTERS as u64)
            .await;

        assert!(mempool.is_full_proposals());
    }

    // Updating subusers should make a partially populated proposal full.
    #[tokio::test]
    async fn update_subusers_completes_partial_proposal() {
        let mempool = Mempool::new();

        mempool.add_proposal(proposal(), 1).await;
        mempool.update_subusers(EXPECTED_VOTERS as u64 - 1);

        assert!(mempool.is_full_proposals());
    }

    // Full proposal notification should wake a waiting task.
    #[tokio::test]
    async fn full_proposal_waiter_is_notified() {
        let mempool = Arc::new(Mempool::new());
        let waiter = Arc::clone(&mempool);

        let waiting = tokio::spawn(async move {
            waiter.wait_for_full_proposal().await;
        });

        mempool
            .add_proposal(proposal(), EXPECTED_VOTERS as u64)
            .await;

        timeout(Duration::from_secs(2), waiting)
            .await
            .expect("proposal notification deadlocked")
            .expect("proposal waiter panicked");
    }

    // A proposal should be replaceable through the normal getter and tally API.
    #[tokio::test]
    async fn latest_added_proposal_is_retrieved() {
        let mempool = Mempool::new();
        let first_owner = Owner::new_sig();
        let second_owner = Owner::new_sig();
        let first = Proposal::new(
            Proposer::new(&first_owner, Block::genesis()),
            HashMap::new(),
        );
        let second = Proposal::new(
            Proposer::new(&second_owner, Block::genesis()),
            HashMap::new(),
        );
        let second_pk = second_owner.pk().to_vec();

        mempool.add_proposal(first, 1).await;
        mempool.add_proposal(second, 1).await;

        let retrieved = mempool.get_proposal().expect("proposal should exist");
        assert_eq!(retrieved.proposer().pk(), second_pk);
        assert_eq!(
            mempool.cmp_proposers(retrieved.proposer()),
            std::cmp::Ordering::Equal,
        );
    }

    // A validator matching the retained proposal should survive clear_validators,
    // while a validator for another block should be removed.
    #[tokio::test]
    async fn clear_validators_retains_only_votes_for_current_proposal() {
        let mempool = Mempool::new();
        let owner = Owner::new_sig();

        let retained_proposer = Proposer::new(&owner, Block::genesis());
        let retained_proposal = Proposal::new(retained_proposer.clone(), HashMap::new());
        mempool.add_proposal(retained_proposal, 1).await;
        mempool
            .add_validator(validated_for(&retained_proposer))
            .await
            .expect("retained validator should be accepted");

        let next_header = BlockHeader::new(
            &BlockHeader::genesis(),
            &BlockBody::new(Vec::new()),
        );
        let next_block = Block::new(
            &next_header,
            BlockBody::new(Vec::new()),
        );
        let mismatched_proposer = Proposer::new(&owner, next_block);
        mempool
            .add_validator(validated_for(&mismatched_proposer))
            .await
            .expect("mismatched validator should be accepted");

        mempool.clear_validators();

        let validators = mempool.get_validators();
        assert_eq!(validators.len(), 1);
        assert!(validators
            .members()
            .all(|vote| vote.as_vote() == retained_proposer.block().id()));
    }

    // Duplicate validators should be rejected without deadlock or a second insertion.
    #[tokio::test]
    async fn duplicate_validator_is_rejected() {
        let mempool = Mempool::new();
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());
        let validated = validated_for(&proposer);

        mempool
            .add_validator(validated.clone())
            .await
            .expect("first validator should be accepted");

        let duplicate = mempool
            .add_validator(validated)
            .await;

        assert!(duplicate.is_err());
        assert_eq!(mempool.get_validators().len(), 1);
    }

    // Concurrent proposals and clears must finish and must not leave the mempool
    // mutex or notification lock in an inaccessible state.
    #[tokio::test]
    async fn concurrent_operations_make_progress_without_deadlock() {
        let mempool = Arc::new(Mempool::new());
        let mut tasks = Vec::new();
        let (sender, receiver) = mpsc::channel();

        for _ in 0..16 {
            let mempool = Arc::clone(&mempool);
            let sender = sender.clone();
            tasks.push(thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new()
                    .expect("failed to create Tokio runtime");

                runtime.block_on(async {
                    mempool.add_proposal(proposal(), 1).await;
                    assert!(mempool.get_proposal().is_some());

                    mempool.update_subusers(1);
                    let _ = mempool.cmp_proposers(proposal().proposer());
                    mempool.clear().await;
                });

                sender.send(()).expect("concurrency test receiver dropped");
            }));
        }
        drop(sender);

        for _ in 0..16 {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("concurrent mempool operations deadlocked");
        }
        for task in tasks {
            task.join().expect("concurrent mempool task panicked");
        }

        mempool.clear().await;
        assert!(mempool.get_proposal().is_none());
        assert!(mempool.get_validators().members().next().is_none());
    }
}
