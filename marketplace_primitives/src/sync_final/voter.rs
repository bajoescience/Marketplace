pub mod proposer;
pub mod validator;

use marketplace_helpers::objects::{AgentResult, ID};
use marketplace_wallet::Lock;

use crate::sync_final::BlockHeader;

pub use self::proposer::{Proposal, Proposer};
pub use self::validator::{Validated, Validator};

pub trait Voter {
    fn owner(&self) -> Lock;

    fn get_vrf_opt(&self) -> AgentResult<ID>;

    // Get block header referenced
    fn header(&self) -> BlockHeader;
}

#[cfg(test)]
mod tests {
    use marketplace_wallet::Owner;

    use crate::sync_final::{Block, Proposer, voter::Voter};

    // The Voter trait should expose the proposer identity and VRF result.
    #[test]
    fn proposer_implements_voter_accessors() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());

        assert_eq!(proposer.owner(), owner.as_lock());
        assert!(proposer.get_vrf_opt().is_ok());
    }
}
