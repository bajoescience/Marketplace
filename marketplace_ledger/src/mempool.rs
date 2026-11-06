mod proposed;

pub use self::proposed::ProposedBlock;

use std::{cmp::Ordering, todo};

use marketplace_primitives::{
    par_exec::Whiteroom, sync_final::{Proposal, Proposer, Validated, blockinfo::BlockInfo},
};

use marketplace_helpers::{functions, objects::{AgentResult}};
use marketplace_wallet::voting::EXPECTED_VOTERS;

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

pub struct Mempool {
    // Handle proposal
    proposed: ProposedBlock, 

    // Validators
    validators: Whiteroom<Validated, WHITEROOM_MAX>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            proposed: ProposedBlock { proposal: None, proposer_count: 0 },
            validators: Whiteroom::new(),
        }
    }

    /// Add a proposal to the MempoolState and update total subusers count
    pub fn add_proposal(&mut self, proposal: Proposal, subusers: u64) {
        self.proposed.add(proposal, subusers);
        
    }

    // Update proposal count
    pub fn update_subusers(&mut self, subusers: u64) {
        self.proposed.update_subusers(subusers);
    }

    /// Get winning proposal to validate to network
    pub fn get_proposal_to_finalize(&mut self) -> Option<Proposal> {
        self.proposed.get_proposal()
    }

    /// Compare blocks to find which one is to be proposed
    pub fn cmp_proposers(&self, proposer: &Proposer) -> Ordering {
        self.proposed.cmp_block_gdc(proposer)
    }

    /// Get block info to put in blockchain
    pub fn get_block_info() -> AgentResult<BlockInfo> {
        todo!()
    }

    /// Add validator to mempool
    pub fn add_validator(&mut self, validated: Validated) -> AgentResult<()> {
        self.validators.add_member(validated)?;
        Ok(())
    }

    /// Clear Mempool entries and input round number 
    /// which is latest block index + 1
    pub fn clear(&mut self) {
        self.proposed = ProposedBlock { proposal: None, proposer_count: 0 };
        self.validators = Whiteroom::new();
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
        let mut mempool = Mempool::new();

        assert!(mempool.get_proposal_to_finalize().is_none());
    }

    // Adding a proposal should make it available for finalization.
    #[test]
    fn added_proposal_can_be_retrieved() {
        let mut mempool = Mempool::new();

        mempool.add_proposal(proposal(), 2);

        assert!(mempool.get_proposal_to_finalize().is_some());
    }

    // Clearing the mempool should remove its current proposal and validator votes.
    #[test]
    fn clear_removes_the_current_proposal() {
        let mut mempool = Mempool::new();
        mempool.add_proposal(proposal(), 2);

        mempool.clear();

        assert!(mempool.get_proposal_to_finalize().is_none());
    }
}
