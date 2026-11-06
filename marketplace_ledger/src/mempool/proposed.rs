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