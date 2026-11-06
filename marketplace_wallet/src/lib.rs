//! # Marketplace Wallet
//! 
//! This library handles Identity and Ownership of assets on the marketplace network.

pub mod wallet;
pub mod owner;
pub mod crypto;
pub mod voting;

pub use wallet::Wallet;
pub use owner::{Lock, Key, Owner};
pub use crypto::Crypto;

pub use marketplace_helpers as helpers;

#[cfg(test)]
mod tests {
    use super::{Crypto, Lock, Owner, Wallet};

    // The crate root should expose the primary wallet, identity, lock, and crypto types.
    #[test]
    fn public_wallet_types_are_available() {
        let owner = Owner::new_sig();
        let wallet = Wallet::new();
        let crypto = Crypto::new(&owner);
        let lock = owner.as_lock();

        assert!(matches!(lock, Lock::SIG(_)));
        assert_eq!(owner.pk().len(), 32);
        assert_eq!(wallet.pubkey_bytes().len(), 32);
        drop(crypto);
    }
}
