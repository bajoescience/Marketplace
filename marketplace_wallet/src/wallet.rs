use std::{error::Error, format, fs::File, io::{Read, Write}};

use marketplace_helpers::objects::AgentResult;
use rand::rngs::OsRng;

use schnorrkel::{KEYPAIR_LENGTH, Keypair, SecretKey};

use crate::{helpers::{functions, objects::{Hash, ID}}, owner::Key};

pub struct Wallet {
    key: Keypair
}

impl Wallet {
    // Create a new Wallet
    // Initialize new assymetric key pair
    pub fn new() -> Self {
        let keypair = Keypair::generate_with(OsRng);

        Self {
            key: keypair
        }
    }

    // New key in file
    pub fn new_keypair_file() -> Result<(), Box<dyn Error>> {
        let wallet = Wallet::new();

        // Serialize to raw bytes (secret key + public key)
        let bytes: [u8; KEYPAIR_LENGTH] = wallet.keypair().to_bytes();

        let mut file = File::create("keypair.bin")?;
        file.write_all(&bytes)?;

        Ok(())
    }

    // Build from keypair file
    pub fn keypair_from_file(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();

        file.read_to_end(&mut bytes)?;

        // Get keypair
        let keypair = Keypair::from_bytes(&bytes).expect("Error converting bytes");
        Ok(Self { key: keypair })
    }

    // Create a new wallet with an existing private key
    pub fn build_from(keypair_byte: &[u8; KEYPAIR_LENGTH]) -> AgentResult<Self> {
        let Ok(signing_key) = Keypair::from_bytes(keypair_byte) else {
            return Err(format!(
                "Error: Couldn't convert keypair bytes to Wallet"
            ))
        };

        Ok(Self { 
            key: signing_key
        })
    }

    // Get VRF
    pub fn keypair(&self) -> &Keypair {
        &self.key
    }

    // Get secret key
    pub fn secret(&self) -> &SecretKey {
        &self.key.secret
    }

    // Sign a message
    pub fn sign(&self, msg: &[u8]) -> Key {
        // TODO: Optionally pad message hash
        let context = schnorrkel::signing_context(b"");
        let sig = self.key.sign(context.bytes(msg));

        // Public key
        let pub_key = self.pubkey_bytes();

        // object to verify message
        Key::SIG(sig.to_bytes(), pub_key)
    }

    // Public key
    pub fn pubkey_bytes(&self) -> ID {
        self.key.public.to_bytes()
    }

    // Get wallet address by hashing public key
    // and adding a checksum
    pub fn address(&self) -> Vec<u8> {
        let addr = self.hash();

        // Get checksum by hashing address
        // and taking first 4 bytes
        functions::generate_address_and_checksum(&addr)
    }
}

// TODO: Create Wallet address by hashing public key
// and adding a checksum.
impl Hash for Wallet {
    fn hash(&self) -> ID {
        let pub_key = self.pubkey_bytes();
        functions::hash(&pub_key)
    }
}



#[cfg(test)]
mod tests {
    use std::{assert_eq, error::Error, panic};

use schnorrkel::{PublicKey, Signature};

use super::*;

    // Initialize new message
    fn message() -> ID {
        [67u8; 32]
    }

    #[test]
    fn sign_using_wallet() -> Result<(), Box<dyn Error + 'static>> {
        let wallet = Wallet::new();

        // Sign message
        let Key::SIG(sig, pubkey) = wallet.sign(&message()) else {
            panic!("Error Signing message");
        };

        let sig = Signature::from_bytes(&sig).unwrap();

        // Confirm signature
        let verifying_key = PublicKey::from_bytes(&pubkey).unwrap();

        let ctx = schnorrkel::signing_context(b"");
        assert!(verifying_key.verify(ctx.bytes(&message()), &sig).is_ok());

        Ok(())
    }
}