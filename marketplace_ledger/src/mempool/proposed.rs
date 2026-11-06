use std::cmp::Ordering;

use marketplace_primitives::sync_final::{Proposal, Proposer};

pub struct ProposedBlock {
    // Winning Proposed block
    pub proposal: Option<Proposal>,

    // Total proposal subuser count
    pub proposer_count: u64,
}

impl ProposedBlock {
    pub fn add(&mut self, proposal: Proposal, subusers: u64) {
        self.proposal = Some(proposal);
        self.proposer_count += subusers;
    }

    // Update proposal count
    pub fn update_subusers(&mut self, subusers: u64) {
        self.proposer_count += subusers;
    }

    // Compare new gdc of blocks, if proposer gdc is greater
    // the method returns Ordering::Greater
    // if less, returns Ordering::Less
    // if both contain the same blocks, method returns Ordering::Equal
    pub fn cmp_block_gdc(&self, proposer: &Proposer) -> Ordering {
        match &self.proposal {
            Some(current) => {
                // The proposal with block having the greater gdc is chosen as the main proposal
                let ordering = proposer.block().header().total_gdc().partial_cmp(
                    &current.block().header().total_gdc()
                ).unwrap();

                if let Ordering::Equal = ordering {
                    // Comapre block ids, the one with lower block id is considered greater
                    let proposer_id = proposer.block().id();
                    let current_id = current.block().id();

                    if proposer_id < current_id {
                        return Ordering::Greater;
                    } else if proposer_id > current_id {
                        return  Ordering::Less;
                    } else {
                        return Ordering::Equal;
                    }
                }

                return ordering;
            },
            None => {
                Ordering::Greater
            }
        }
    }

    // Clear the struct and return a proposal
    pub fn get_proposal(&mut self) -> Option<Proposal> {
        self.proposal.clone()
    }
}

#[cfg(test)]
mod tests {
    use marketplace_primitives::sync_final::{Block, Proposer, Proposal};
    use marketplace_wallet::Owner;

    use super::ProposedBlock;

    fn proposal() -> Proposal {
        let owner = Owner::new_sig();
        Proposal::new(Proposer::new(&owner, Block::genesis()), Default::default())
    }

    // Adding proposals should retain the latest proposal and accumulate its proposer count.
    #[test]
    fn add_stores_proposal_and_accumulates_count() {
        let mut proposed = ProposedBlock {
            proposal: None,
            proposer_count: 0,
        };
        let proposal = proposal();

        proposed.add(proposal, 3);

        assert_eq!(proposed.proposer_count, 3);
        assert!(proposed.get_proposal().is_some());
    }

    // Additional proposer support should increase the tally without replacing the proposal.
    #[test]
    fn update_subusers_changes_only_the_count() {
        let mut proposed = ProposedBlock {
            proposal: Some(proposal()),
            proposer_count: 4,
        };

        proposed.update_subusers(5);

        assert_eq!(proposed.proposer_count, 9);
        assert!(proposed.get_proposal().is_some());
    }

    // The initial proposal candidate should outrank the empty proposed-block state.
    #[test]
    fn candidate_outranks_empty_proposal_state() {
        let proposed = ProposedBlock {
            proposal: None,
            proposer_count: 0,
        };
        let candidate = proposal();

        assert_eq!(
            proposed.cmp_block_gdc(candidate.proposer()),
            std::cmp::Ordering::Greater
        );
    }

    // A block identical to the current proposal should compare as equal.
    #[test]
    fn identical_candidate_compares_equal() {
        let proposal = proposal();
        let mut proposed = ProposedBlock {
            proposal: Some(proposal.clone()),
            proposer_count: 1,
        };

        assert_eq!(
            proposed.cmp_block_gdc(proposal.proposer()),
            std::cmp::Ordering::Equal
        );
    }
}