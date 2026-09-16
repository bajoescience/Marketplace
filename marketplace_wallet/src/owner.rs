use std::{format, hash::Hash as StdHash};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::objects::{AgentResult, Hash, IdHash};
use schnorrkel::{KEYPAIR_LENGTH, Keypair, PublicKey, SIGNATURE_LENGTH, Signature};
use crate::{Wallet, helpers::{functions, objects::ID}};

/// # Owner
/// 
/// `Owner` is an identity that can interact with assets on the marketplace by locking and unlocking 
/// marketplace primitives which include:
/// 
/// - `Work Pointer`
/// 
/// - `Result Pointer`
/// 
/// - `Transactions`
/// 
/// when locking these primitives, the `Lock` generates a `Key` that can be used to later prove ownership of the primitives.
/// 
/// **There are multiple types of Owners which include:**
/// 
/// ## SIG
/// 
/// `SIG` is an `Owner` that locks a primitive by signing it
/// with a single `Wallet`.
/// 
/// ### Examples
/// 
/// Initialize a new `SIG Owner`
/// and sign a message
/// 
/// ```
/// use marketplace_wallet::Owner;
/// 
/// let msg = [20u8; 32];
/// 
/// let owner = Owner::new_sig();
/// 
/// // Sign message
/// let key = owner.sign(&msg);
/// 
/// // Lock form of owner
/// let lock = owner.as_lock();
/// 
/// // Prove signature
/// assert!(lock.unlock(&key, &msg).is_ok());
/// ```
/// 
pub enum Owner {
    // Single sig containing pubkey hash
    SIG(Wallet),

    // Keeps ID of account/token
    // ACCOUNT(Box<Owner>),
}

impl Owner {
    /// Create a new `Owner` with a single keypair
    pub fn new_sig() -> Self {
        let wallet = Wallet::new();

        Self::SIG(wallet)
    }

    /// Build single `Owner` from secret key bytes by
    /// Create a new wallet with an existing private key
    pub fn build_from(keypair: &[u8; KEYPAIR_LENGTH]) -> AgentResult<Self> {
        let Ok(wallet) = Wallet::build_from(keypair) else {
            return Err(format!(
                "Error: Invalid Keypair"
            ))
        };

        Ok(Self::SIG(wallet))
    }

    // Return an owner's keypair
    pub fn keypair(&self) -> &Keypair {
        match self {
            Self::SIG(wallet) => wallet.keypair()
        }
    }

    // Return owner's public key
    pub fn pk(&self) -> ID {
        match self {
            Owner::SIG(wallet) => wallet.pubkey_bytes()
        }
    }

    // Create a new account owner
    // pub fn new_account(owner: Owner) -> Self {
    //     Self::ACCOUNT(Box::new(owner))
    // }

    /// This returns the Lock version of Owner isntance 
    /// For example:
    /// 
    /// Owner::SIG(...bytes);
    /// 
    /// can simply be written in lock form as 
    /// Lock::SIG(hash(...bytes))
    /// 
    /// Which is a better form for transactions, but both are the same
    
    // Return lock object synonymous with owner
    pub fn as_lock(&self) -> Lock {
        match self {
            Self::SIG(wallet) => Lock::SIG(wallet.hash()),
        }
    }

    /// Return whiteroom lock that claims whiteroom pay
    pub fn as_wr_lock(&self) -> Lock {
       match self {
           Self::SIG(wallet) => Lock::WHITEROOM(wallet.hash()),
       } 
    }

    pub fn sign(&self, msg: &[u8]) -> Key {
        match self {
            Self::SIG(wallet) => wallet.sign(msg),
        }
    }
}

// This is just to show that both are the same
// but appear in different forms
impl PartialEq<Lock> for Owner {
    fn eq(&self, other: &Lock) -> bool {
        // To be equal, they must have the same variant
        match (self, other) {
            (Self::SIG(wallet), Lock::SIG(pubkey_hash)) => {
                wallet.hash() == *pubkey_hash
            },
            (Self::SIG(wallet), Lock::WHITEROOM(wr_owner)) => {
                wallet.hash() == *wr_owner
            },
            _ => false
        }
    }
}

// We change the owner to it's lock form
// before getting it's ID
impl IdHash for Owner {
    fn id(&self) -> ID {
        self.as_lock().id()
    }
}

// Compare two owners
impl PartialEq for Owner {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Owner {}

impl StdHash for Owner {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        self.id();
    }
}


/// # Lock
/// 
/// `Lock` is a type that enables ownership of a marketplace primitive by locking it. Only a corresponding `Owner`
/// can unlock it using a `Key`.
/// 
/// **There are multiple types of Lock which include:**
/// 
/// ## SIG
/// 
/// `SIG` contains a single `Wallet` hash (public key hash) of the form
/// 
/// ### Examples
/// 
/// ```
/// use marketplace_wallet::{Lock, Wallet};
/// use marketplace_helpers::objects::Hash;
/// 
/// let wallet = Wallet::new();
/// let lock = Lock::SIG(wallet.hash());
/// ```
/// 
/// or using the standard `Owner` `SIG`
/// 
/// ```
/// use marketplace_wallet::{Lock, Owner};
/// 
/// let owner = Owner::new_sig();
/// let lock = owner.as_lock();
/// ```
/// 
/// **Using a Key, we can unlock a locked primitive**
/// 
/// ```
/// use marketplace_wallet::{Lock, Owner};
/// 
/// // primitive hash
/// let msg = [20u8; 32];
/// 
/// let owner = Owner::new_sig();
/// let lock = owner.as_lock();
/// 
/// let key = owner.sign(&msg);
/// 
/// // Unlock primitive
/// assert!(lock.unlock(&key, &msg).is_ok());
/// ```
/// 
/// ## WHITEROOM
/// 
/// `WHITEROOM` is a type of `Lock` for `Whiteroom` members in a `Job`. It indicates to the network
/// that the owner is a whiteroom member.
/// 
/// ### Examples
/// 
/// ```
/// use marketplace_wallet::{Lock, Owner};
/// // primitive hash
/// let msg = [20u8; 32];
/// 
/// let owner = Owner::new_sig();
/// 
/// // Note: Whiteroom lock method is different from 
/// // normal lock method
/// let lock = owner.as_wr_lock();
/// 
/// let key = owner.sign(&msg);
/// 
/// // Unlock primitive
/// assert!(lock.unlock(&key, &msg).is_ok());
/// ```
/// 
/// ## ACCOUNT
/// 
/// `ACCOUNT` is a type of `Lock` that proves that a token account owns an asset. They
/// are a special type of lock 
#[derive(Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum Lock {
    // Single sig containing pubkey hash
    SIG(ID),

    // Each whiteroom member owns the full value locked by the whiteroom
    // this lock contains the owner id of the whiteroom
    WHITEROOM(ID),

    // Keeps ID of account/token
    // Which is the hash of a transactions
    // account id
    ACCOUNT(ID),
}

fn unlock(
    msg_id: &[u8],
    pubkey_hash: &[u8],
    sig: &[u8],
    pubkey: &[u8],
) -> AgentResult<()> {

    // Validate pubkey
    if functions::hash(pubkey) != *pubkey_hash {
        return Err(format!(
            "Error: Public key does not match Stored hash"
        ));
    }

    // Convert SIG from bytes
    let Ok(sig) = Signature::from_bytes(sig) else {
        return Err(format!(
            "Error: Signature could not be fetched"
        ));
    };

    // Get verifying key from pubkey
    let Ok(verifying_key) = PublicKey::from_bytes(pubkey) else {
        return Err(format!(
            "Error: Public key error"
        ));
    };

    let ctx = schnorrkel::signing_context(b"");

    let Ok(_) = verifying_key.verify(ctx.bytes(msg_id), &sig) else {
        return Err(format!(
            "Error: Could not verify key"
        ));
    };

    Ok(())
}

impl Lock {
    pub fn unlock(&self, key: &Key, msg_id: &ID) -> AgentResult<()> {
        match (self, key) {
            (Self::SIG(pubkey_hash), Key::SIG(sig, pubkey)) => {
                unlock(msg_id, pubkey_hash, sig, pubkey)
            },
            (Self::WHITEROOM(wr_owner), Key::SIG(sig, pubkey)) => {
                unlock(msg_id, wr_owner, sig, pubkey)
            }
            _=> Ok(())
        }
    }

    // Create a new acc_id
    pub fn acc(tid: ID) -> Self {
        Self::ACCOUNT(tid)
    } 
}

/// # Key
/// `Key` is used to unlock a `Lock` type and prove ownership of a locked primitive.
/// 
/// **see `Lock`**
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    // (Signature, pubkey)
    SIG([u8; SIGNATURE_LENGTH], ID),

    ACCOUNT,
}


// TODO: Whiteroom Signature

#[cfg(test)]
mod tests {

use marketplace_helpers::{objects::{Hash}};

use crate::wallet::Wallet;
    use super::*;

    fn message() -> ID {
        functions::dum_bytes()
    }

    // Whiteroom Sig
    #[test]
    fn whiteroom_sig() {
        // Whiteroom owner
        let wr_owner = Owner::new_sig();

        // Whiteroom lock
        let wr_lock = wr_owner.as_wr_lock();

        // Wallet signs message
        wr_owner.sign(&message());
    }

    // Single sig
    #[test]
    fn single_sig() {
        let owner = Owner::new_sig();

        // Wallet signs message
        let key = owner.sign(&message());

        // Create Sig instance
        let lock = owner.as_lock();

        assert!(lock.unlock(&key, &message()).is_ok())
    }

    // Sinlge sig should fail
    #[test]
    fn single_sig_fail() {
        let wallet = Wallet::new();

        // Wallet signs message
        let key = wallet.sign(&message());

        // Create Sig instance
        let owner = Lock::SIG(wallet.hash());

        // Use invalid message id
        assert!(owner.unlock(&key, &[67; 32]).is_err())

    }
}