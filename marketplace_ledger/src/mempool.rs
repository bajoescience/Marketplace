mod proposed;

pub use self::proposed::ProposedBlock;

use std::{cmp::Ordering, sync::{Mutex, MutexGuard}, todo};
use tokio::sync::Notify;

use marketplace_primitives::{
    par_exec::Whiteroom, sync_final::{Proposal, Proposer, Validated, blockinfo::BlockInfo},
};

use marketplace_helpers::{functions, objects::{AgentResult}};
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

    fn clear(&mut self) {
        self.proposed = ProposedBlock::new();
        self.validators = Whiteroom::new();
    }

    pub fn clear_validators(&mut self) {
        self.validators = Whiteroom::new();
    }
}

pub struct Mempool {

    // Notify object to notify when proposal is full
    proposer_notify: Notify,

    // Notify object to notify when validator has reach consensus
    validator_notify: Notify,

    // Notify when validator cannot reach consensus
    validator_not_notify: Notify,

    // Sync final lists
    sync_lists: Mutex<SyncLists>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            proposer_notify: Notify::new(),
            validator_notify: Notify::new(),
            validator_not_notify: Notify::new(),
            sync_lists: Mutex::new(SyncLists::new()),
        }
    }

    /// Lock proposal and validators as sync lists to be able to access
    /// method returns (proposed_lock, validators_lock)
    fn sync_lists(&self) -> MutexGuard<'_, SyncLists> {
        self.sync_lists.lock().unwrap()
    }

    /// Clear Mempool sync lists entries
    pub fn clear(&self) {
        self.sync_lists().clear()
    }

    /// Clear only validator sync list
    pub fn clear_validators(&self) {
        self.sync_lists().clear_validators();
    }
}

// Async methods 
// some check for full proposal or validator list
impl Mempool {
    /// If proposal list is full, get the proposal
    /// to be validated
    pub async fn wait_for_full_proposal(&self) {
        self.proposer_notify.notified().await;
    }

    /// If validated list has reached consensus, get blockinfo
    pub async fn wait_for_consensus_validated(&self) -> BlockInfo {
        self.validator_notify.notified().await;

        self.get_blockinfo().await
    }

    /// If validated, do something real.
    pub async fn wait_for_failed_consensus_validated(&self) {
        self.validator_not_notify.notified().await;
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
    pub fn add_proposal(&self, proposal: Proposal, subusers: u64) {
        let proposed = &mut self.sync_lists().proposed;

        proposed.add(proposal, subusers);

        // Check if proposal is full
        if proposed.is_full() {
            self.proposer_notify.notify_one();
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
    pub fn add_validator(&self, validated: Validated) -> AgentResult<()> {
        let validators = &mut self.sync_lists().validators;

        validators.add_member(validated)?;

        // Check if validators has reached consensus
        if validators.is_consensus() {
            self.validator_notify.notify_one();
        }

        // Check if validators can reach consensus
        if !validators.can_consensus() {
            self.validator_not_notify.notify_one();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use marketplace_primitives::sync_final::{Block, Proposer, Proposal};
    use marketplace_wallet::Owner;

    use super::Mempool;

    fn proposal() -> Proposal {
        let owner = Owner::new_sig();
        Proposal::new(Proposer::new(&owner, Block::genesis()), Default::default())
    }

    // A new mempool should contain no proposal to finalize.
    #[test]
    fn new_mempool_has_no_proposal() {
        let mempool = Mempool::new();

        assert!(mempool.get_proposal().is_none());
    }

    // Adding a proposal should make it available for finalization.
    #[test]
    fn added_proposal_can_be_retrieved() {
        let mempool = Mempool::new();

        mempool.add_proposal(proposal(), 2);

        assert!(mempool.get_proposal().is_some());
    }

    // Clearing the mempool should remove its current proposal and validator votes.
    #[test]
    fn clear_removes_the_current_proposal() {
        let mempool = Mempool::new();
        mempool.add_proposal(proposal(), 2);

        mempool.clear();

        assert!(mempool.get_proposal().is_none());
    }
}
