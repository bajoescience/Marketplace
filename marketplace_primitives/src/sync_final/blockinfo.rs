use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;

use crate::{
    par_exec::Whiteroom, sync_final::{Block, Proposal, Validated},
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
    proposal: Proposal,
    validators: Whiteroom<Validated, WHITEROOM_MAX>
}

impl BlockInfo {
    pub fn new(proposal: Proposal, validators: Whiteroom<Validated, WHITEROOM_MAX>) -> Self {
        Self {
            proposal,
            validators
        }
    }
}

// Getter methods
impl BlockInfo {
    // Get proposal
    pub fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    // Get block
    pub fn block(&self) -> &Block {
        &self.proposal().block()
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
        sync::{Arc, Barrier, mpsc},
        thread,
        time::Duration,
    };

    use crate::{
        par_exec::Whiteroom,
        sync_final::{Proposer, Validator},
    };

    fn blockinfo_with_votes(weights: &[u64]) -> BlockInfo {
        let proposer_owner = Owner::new_sig();
        let block = Block::genesis();
        let seed = block.header().prev();
        let proposer = Proposer::new(&proposer_owner, block);
        let mut validators = Whiteroom::<Validated, 33>::new();

        for weight in weights {
            let owner = Owner::new_sig();
            let validator = Validator::new(&owner, &seed, &proposer);
            validators
                .add_member(Validated::new(validator, *weight, HashMap::new()))
                .unwrap();
        }

        BlockInfo::new(
            Proposal::new(proposer, HashMap::new()),
            validators,
        )
    }

    // BlockInfo should expose the proposal's block and the full validator set.
    #[test]
    fn blockinfo_getters_expose_block_and_validators() {
        let info = blockinfo_with_votes(&[2, 3]);
        let block_id = info.proposal().block().id();

        assert_eq!(info.block().id(), block_id);
        assert_eq!(info.proposal().proposer().block().id(), block_id);
        assert_eq!(info.validators().members().count(), 2);
        assert_eq!(info.validators().len(), 5);
        assert_eq!(*info.validators().winning_vote().unwrap().1, 5);
    }

    // A block info object with no validators should still retain its proposal.
    #[test]
    fn blockinfo_allows_an_empty_validator_set() {
        let info = blockinfo_with_votes(&[]);

        assert_eq!(info.block().id(), info.proposal().block().id());
        assert_eq!(info.validators().members().count(), 0);
        assert_eq!(info.validators().len(), 0);
        assert!(info.validators().winning_vote().is_none());
    }

    // Serialization should preserve the block, proposal, and validator vote data.
    #[test]
    fn blockinfo_serialization_round_trip() -> Result<(), std::io::Error> {
        let info = blockinfo_with_votes(&[1, 4, 2]);
        let bytes = IdHash::serialize(&info);
        let restored = BlockInfo::from_bytes(&bytes)?;

        assert_eq!(restored.block().id(), info.block().id());
        assert_eq!(restored.validators().members().count(), 3);
        assert_eq!(restored.validators().len(), 7);
        assert_eq!(
            restored.validators().winning_vote().unwrap(),
            info.validators().winning_vote().unwrap(),
        );
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
        let expected_block_id = info.block().id();
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
                    assert_eq!(cloned.block().id(), expected_block_id);
                    assert_eq!(cloned.validators().len(), expected_votes);

                    let restored = BlockInfo::from_bytes(
                        &IdHash::serialize(&cloned),
                    ).unwrap();
                    assert_eq!(restored.block().id(), expected_block_id);
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