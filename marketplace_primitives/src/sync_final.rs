/// ## Synchronous Finalization
pub mod block;
pub mod proposal;
pub mod validator;

pub use self::block::{BlockHeader, Block};
pub use self::proposal::{Proposer};
pub use self::validator::{Validator, Validated};