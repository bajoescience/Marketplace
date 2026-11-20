use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;

use crate::{
    par_exec::Whiteroom,
    sync_final::{voter::Voter, Block, BlockHeader, Proposal, Validated},
};

use marketplace_wallet::voting::EXPECTED_VOTERS;

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

/// Full information concerning a `Block` which includes
/// the following:
///
/// - Block
///
/// - List of Validators
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BlockInfo {
    proposal: Option<Proposal>,
    validators: Whiteroom<Validated, WHITEROOM_MAX>,
}

impl BlockInfo {
    pub fn new(
        proposal: Option<Proposal>,
        validators: Whiteroom<Validated, WHITEROOM_MAX>,
    ) -> Self {
        Self {
            proposal,
            validators,
        }
    }
}

// Getter methods
impl BlockInfo {
    // Get proposal
    pub fn proposal(&self) -> Option<&Proposal> {
        self.proposal.as_ref()
    }

    // Get block
    pub fn block(&self) -> Option<&Block> {
        self.proposal().map(|proposal| proposal.block())
    }

    // Get header agreed upon by validators
    pub fn header(&self) -> Option<BlockHeader> {
        if !self.validators().is_consensus() {
            return None;
        }

        self.validators()
            .winners()
            .next()
            .map(|val| val.validator().header())
    }

    // Get Validators
    pub fn validators(&self) -> &Whiteroom<Validated, WHITEROOM_MAX> {
        &self.validators
    }
}

#[cfg(test)]
mod tests {
    use marketplace_helpers::objects::IdHash;
    use marketplace_wallet::Owner;

    use super::*;
    use std::{
        collections::HashMap,
        sync::{mpsc, Arc, Barrier},
        thread,
        time::Duration,
    };

    use crate::{
        par_exec::Whiteroom,
        sync_final::{BlockBody, Proposer, Validator},
    };

    fn blockinfo_with_validator_headers(
        proposal: Option<Proposal>,
        votes: &[(BlockHeader, u64)],
    ) -> BlockInfo {
        let mut validators = Whiteroom::<Validated, WHITEROOM_MAX>::new();

        for (header, weight) in votes {
            let owner = Owner::new_sig();
            let validator = Validator::new(&owner, *header);
            validators
                .add_member(Validated::new(validator, *weight, HashMap::new()))
                .unwrap();
        }

        BlockInfo::new(proposal, validators)
    }

    fn blockinfo_with_votes(weights: &[u64]) -> BlockInfo {
        let proposer_owner = Owner::new_sig();
        let block = Block::genesis();
        let header = *block.header();
        let proposer = Proposer::new(&proposer_owner, block);
        let mut validators = Whiteroom::<Validated, 33>::new();

        for weight in weights {
            let owner = Owner::new_sig();
            let validator = Validator::new(&owner, header);
            validators
                .add_member(Validated::new(validator, *weight, HashMap::new()))
                .unwrap();
        }

        BlockInfo::new(Some(Proposal::new(proposer, HashMap::new())), validators)
    }

    // BlockInfo should expose the proposal's block and the full validator set.
    #[test]
    fn blockinfo_getters_expose_block_and_validators() {
        let info = blockinfo_with_votes(&[2, 3]);
        let block_id = info.proposal().unwrap().block().id();

        assert_eq!(info.block().unwrap().id(), block_id);
        assert_eq!(info.proposal().unwrap().proposer().block().id(), block_id);
        assert_eq!(info.validators().members().count(), 2);
        assert_eq!(info.validators().len(), 5);
        assert_eq!(*info.validators().winning_vote().unwrap().1, 5);
    }

    // BlockInfo should retain the absence of a proposal.
    #[test]
    fn blockinfo_allows_no_proposal() {
        let validators = Whiteroom::<Validated, 33>::new();
        let info = BlockInfo::new(None, validators);

        assert!(info.proposal().is_none());
        assert!(info.block().is_none());
        assert_eq!(info.validators().len(), 0);
        assert_eq!(info.validators().members().count(), 0);
    }

    // A block info object with no validators should still retain its proposal.
    #[test]
    fn blockinfo_allows_an_empty_validator_set() {
        let info = blockinfo_with_votes(&[]);

        assert_eq!(
            info.block().unwrap().id(),
            info.proposal().unwrap().block().id(),
        );
        assert_eq!(info.validators().members().count(), 0);
        assert_eq!(info.validators().len(), 0);
        assert!(info.validators().winning_vote().is_none());
    }

    // An empty whiteroom must not produce a header.
    #[test]
    fn blockinfo_header_is_none_without_validators() {
        let info = blockinfo_with_validator_headers(None, &[]);

        assert!(!info.validators().is_consensus());
        assert!(info.header().is_none());
    }

    // A winner below the BFT threshold must not produce a header.
    #[test]
    fn blockinfo_header_is_none_below_consensus_threshold() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let header = BlockHeader::genesis();
        let info = blockinfo_with_validator_headers(None, &[(header, (threshold - 1) as u64)]);

        assert!(!info.validators().is_consensus());
        assert!(info.header().is_none());
    }

    // A header must be returned when the winning vote reaches the threshold exactly.
    #[test]
    fn blockinfo_header_is_some_at_exact_consensus_threshold() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let header = BlockHeader::genesis();
        let info = blockinfo_with_validator_headers(None, &[(header, threshold as u64)]);

        assert!(info.validators().is_consensus());
        assert_eq!(info.header().unwrap().id(), header.id());
    }

    // A header must be returned above the consensus threshold.
    #[test]
    fn blockinfo_header_is_some_above_consensus_threshold() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let header = BlockHeader::genesis();
        let info = blockinfo_with_validator_headers(None, &[(header, (threshold + 1) as u64)]);

        assert!(info.validators().is_consensus());
        assert_eq!(info.header().unwrap().id(), header.id());
    }

    // Two tied headers below the threshold must not produce any header.
    #[test]
    fn blockinfo_header_is_none_for_tied_below_threshold_votes() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let first = BlockHeader::genesis();
        let second = BlockHeader::new(&first, &BlockBody::new(Vec::new()));
        let split = (threshold - 1) as u64;
        let info = blockinfo_with_validator_headers(None, &[(first, split), (second, split)]);

        assert!(!info.validators().is_consensus());
        assert!(info.header().is_none());
    }

    // When votes are split, only the header reaching consensus may be returned.
    #[test]
    fn blockinfo_header_is_consensus_winner_when_votes_are_split() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let winning = BlockHeader::genesis();
        let losing = BlockHeader::new(&winning, &BlockBody::new(Vec::new()));
        let info =
            blockinfo_with_validator_headers(None, &[(winning, threshold as u64), (losing, 1)]);

        assert!(info.validators().is_consensus());
        assert_eq!(info.header().unwrap().id(), winning.id());
    }

    // A split vote with no header reaching the threshold must not return a header.
    #[test]
    fn blockinfo_header_is_none_when_split_below_consensus() {
        let threshold = functions::bft_thresh(EXPECTED_VOTERS);
        let first = BlockHeader::genesis();
        let second = BlockHeader::new(&first, &BlockBody::new(Vec::new()));
        let first_votes = threshold - 1;
        let second_votes = threshold - 2;
        let info = blockinfo_with_validator_headers(
            None,
            &[(first, first_votes as u64), (second, second_votes as u64)],
        );

        assert!(!info.validators().is_consensus());
        assert!(info.header().is_none());
    }

    // Cloning BlockInfo should preserve its proposal, block, and validator set.
    #[test]
    fn blockinfo_clone_preserves_all_data() {
        let info = blockinfo_with_votes(&[1, 4, 2]);
        let cloned = info.clone();

        assert_eq!(cloned.block().unwrap().id(), info.block().unwrap().id(),);
        assert_eq!(cloned.validators().len(), info.validators().len());
        assert_eq!(cloned.validators().members().count(), 3);
    }

    // Cloning should not allow one validator-set clone to mutate the original.
    #[test]
    fn blockinfo_clone_has_an_independent_validator_set() {
        let info = blockinfo_with_votes(&[1, 2]);
        let mut cloned = info.clone();

        assert_eq!(cloned.validators().len(), 3);
        assert_eq!(cloned.validators.owned_members().count(), 2,);
        assert_eq!(info.validators().len(), 3);
        assert_eq!(info.validators().members().count(), 2);
    }

    // Serialization should preserve the block, proposal, and validator vote data.
    #[test]
    fn blockinfo_serialization_round_trip() -> Result<(), std::io::Error> {
        let info = blockinfo_with_votes(&[1, 4, 2]);
        let bytes = IdHash::serialize(&info);
        let restored = BlockInfo::from_bytes(&bytes)?;

        assert_eq!(restored.block().unwrap().id(), info.block().unwrap().id());
        assert_eq!(restored.validators().members().count(), 3);
        assert_eq!(restored.validators().len(), 7);
        assert_eq!(
            restored.validators().winning_vote().unwrap(),
            info.validators().winning_vote().unwrap(),
        );
        Ok(())
    }

    // Serialization should preserve a BlockInfo without a proposal.
    #[test]
    fn blockinfo_without_proposal_serializes_round_trip() -> Result<(), std::io::Error> {
        let info = BlockInfo::new(None, Whiteroom::<Validated, 33>::new());
        let restored = BlockInfo::from_bytes(&IdHash::serialize(&info))?;

        assert!(restored.proposal().is_none());
        assert!(restored.block().is_none());
        assert_eq!(restored.validators().len(), 0);
        Ok(())
    }

    // Truncated serialized data should be rejected rather than decoded partially.
    #[test]
    fn blockinfo_rejects_truncated_serialized_data() {
        assert!(BlockInfo::from_bytes(&[0, 1, 2]).is_err());
    }

    // Immutable BlockInfo reads, clones, and serialization should be safe under
    // contention and complete without blocking on shared synchronization.
    #[test]
    fn concurrent_reads_and_clones_make_progress() {
        let info = Arc::new(blockinfo_with_votes(&[2, 3]));
        let expected_block_id = info.block().unwrap().id();
        let expected_votes = info.validators().len();
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers + 1));
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();

        for _ in 0..workers {
            let info = Arc::clone(&info);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    let cloned = info.as_ref().clone();
                    assert_eq!(cloned.block().unwrap().id(), expected_block_id);
                    assert_eq!(cloned.validators().len(), expected_votes);

                    let restored = BlockInfo::from_bytes(&IdHash::serialize(&cloned)).unwrap();
                    assert_eq!(restored.block().unwrap().id(), expected_block_id);
                    assert_eq!(restored.validators().len(), expected_votes);
                }
                sender.send(()).unwrap();
            }));
        }
        drop(sender);

        barrier.wait();
        for _ in 0..workers {
            receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("concurrent BlockInfo reads deadlocked");
        }
        for handle in handles {
            handle.join().expect("BlockInfo reader panicked");
        }
    }
}
