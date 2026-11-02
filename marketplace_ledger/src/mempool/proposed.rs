use std::cmp::Ordering;

use marketplace_primitives::sync_final::Proposal;


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

    pub fn add_proposal(&mut self, proposal: Proposal, subusers: u64) {
        match &self.proposal {
            Some(current) => {
                // The proposal with block having the greater gdc is chosen as the main proposal
                match proposal.block().header().total_gdc().partial_cmp(
                    &current.block().header().total_gdc()
                ).unwrap() {
                    Ordering::Greater => {
                        self.add(proposal.clone(), subusers);
                    },
                    Ordering::Equal => {
                        // Comapre block id, the one with lower block id is chosen
                        if proposal.block().id() < current.block().id() {
                            self.add(proposal.clone(), subusers);
                        }
                    },
                    Ordering::Less => {
                        // Just update subusers
                        self.proposer_count += subusers;
                    },
                }
            },
            None => {
                self.add(proposal.clone(), subusers);
            }
        }
    }

    // Clear the struct and return a proposal
    pub fn get_proposal(&mut self) -> Option<Proposal> {
        self.proposal.clone()
    }
}