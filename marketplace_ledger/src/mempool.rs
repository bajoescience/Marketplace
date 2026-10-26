mod proposed;

pub use self::proposed::ProposedBlock;

use std::{cmp::Ordering, todo};

use marketplace_primitives::{Proposal, Validated, Whiteroom, sync_final::blockinfo::BlockInfo};

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
    pub fn add_proposal(&mut self, proposal: &Proposal, subusers: u64) {
        match &self.proposed.proposal {
            Some(winner_proposal) => {
                // The proposal with block having the greater gdc is chosen as the main proposal
                match proposal.block.header().total_gdc().partial_cmp(
                    &winner_proposal.block.header().total_gdc()
                ).unwrap() {
                    Ordering::Greater => {
                        self.proposed.add_proposal(proposal.clone(), subusers);
                    },
                    Ordering::Equal => {
                        // Comapre block id, the one with lower block id is chosen
                        if proposal.block.id() < winner_proposal.block.id() {
                            self.proposed.add_proposal(proposal.clone(), subusers);
                        }
                    },
                    Ordering::Less => {
                        // Just update subusers
                        self.proposed.proposer_count += subusers;
                    },
                }
            },
            None => {
                self.proposed.add_proposal(proposal.clone(), subusers);
            }
        }
    }

    /// Get winning proposal to validate to network
    pub fn get_proposal_to_finalize(&mut self) -> Option<Proposal> {
        self.proposed.get_proposal()
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

