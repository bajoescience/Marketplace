use marketplace_primitives::Proposal;


pub struct ProposedBlock {
    // Winning Proposed block
    pub proposal: Option<Proposal>,

    // Total proposal subuser count
    pub proposer_count: u64,
}

impl ProposedBlock {
    pub fn add_proposal(&mut self, proposal: Proposal, subusers: u64) {
        self.proposal = Some(proposal);
        self.proposer_count += subusers;
    }

    // Clear the struct and return a proposal
    pub fn get_proposal(&mut self) -> Option<Proposal> {
        self.proposal.clone()
    }
}