use std::{todo};

use sha2::{Sha256, Digest};
use time::OffsetDateTime;

use crate::objects::{ID, MIN_WORK_SIZE, VRF_T, WU, WHITEROOM_MAX, VDF_CONSTANT};
use alloy_primitives::{U256};

/// BFT whiteroom size given f amounts of tolerable faulty nodes
pub fn bft_from(f: usize) -> usize {
    (3 * f) + 1
}

/// BFT tolerable number of non-faulty nodes given a whiteroom size m
pub fn bft_thresh(m: usize) -> usize {
    let f = (m - 1) / 3;

    (2 * f) + 1
}

// Get vdf size by multiplying 
// the result of work_size divided by 10000 and
// the result of vrf threshold divided by 2 to 256 power.
pub fn vdf_difficulty(work_size: WU) -> u64 {
    // Divide by a value of 100000 to account for squaring work
    let work_size = work_size.inner() / VDF_CONSTANT;

    let vdf_size = work_size / WHITEROOM_MAX as u128;

    // Convert difficulty to u64
    let result = u64::try_from(vdf_size).unwrap_or(u64::MAX);

    // The absolute minimum difficulty is 1
    std::cmp::max(result, 1)

}

// Calculate average of two WU
// 1 is the lowest number gotten
pub fn avg_wu(a: WU, b: WU) -> WU {
    let value = (a.inner() + b.inner()) / 2;

    if value < 1 {
        WU::single()
    } else {
        WU::try_from(value).unwrap()
    }
}

// Create new VRF threshold using network average and Job work
pub fn get_vrf(network_avg: WU, job_size: WU) -> VRF_T {
    // Max VRF which is also the network initial VRF
    let vrf = U256::from_be_bytes([255u8; 32]);

    let network_avg = U256::from(network_avg.inner());
    let job_size = U256::from(job_size.inner());

    ((vrf / network_avg).saturating_mul(job_size)).to_be_bytes()
}

// Convert a hex_string to bytes
pub fn to_bytes(hex_str: &str) -> Result<ID, Box<dyn std::error::Error>> {
    let mut bytes = [0u8; 32];

    hex::decode_to_slice(hex_str, &mut bytes)?;
    Ok(bytes)
}

pub fn from_bytes(id: &ID) -> String {
    hex::encode(id)
}

// Founder's pub key hash
pub fn founders() -> ID {
    to_bytes(
        "9eec6b485942aeb14e7a7955e1b31e061c26f46cbf5398263e0c3d43235017d5"
    ).unwrap()
}

// Get job fee amount from goldcoin amount
pub fn fee_price(amount: WU) -> WU {
    amount.ten_fold() / MIN_WORK_SIZE
}

// Casual fee amount from goldcoin amount
pub fn casual_fee_price(amount: WU) -> WU {
    amount / MIN_WORK_SIZE.ten_fold()
}

// Get a Whiteroom seed using hash of work_ptr_id and pub key
pub fn wr_seed(work_ptr_id: &ID, pk: &[u8]) -> ID {
    let mut hasher = Sha256::new();
    hasher.update(pk);
    hasher.update(work_ptr_id);

    hasher.finalize().into()
}

// Hash a series of bytes
pub fn hash(bytes: &[u8]) -> ID {
    // Hash bytes with sha256 twice
    Sha256::digest(Sha256::digest(bytes)).into()
}

// Dummy bytes for tests and placeholders
pub fn dum_bytes() -> ID {
    to_bytes("90a9a6923dd9ec246d8046c2bde4d323396f59b2d7a4d43e162db9aeda017b93").unwrap()
}

// Change a raw wallet address to readable address
pub fn generate_address_and_checksum(addr: &ID) -> Vec<u8> {
    let check = hash(addr);
    let checksum = &check[0..4];

    [addr, checksum].concat()
}

// Change a readable wallet address to raw address
pub fn validate_address_and_checksum(addr: Vec<u8>) -> ID {
    let raw_addr = addr;
    todo!()
}

pub fn timestamp() -> u64 {
    OffsetDateTime::now_utc().unix_timestamp() as u64
}


#[cfg(test)]
mod tests {
    use std::{assert_eq, error::Error};

use crate::objects::WU;

use super::*;

    // Test vdf difficulty
    #[test]
    fn test_vdf_diff() {
        let work_size = WU::try_from(3000000000).unwrap();

        let vdf_diff = vdf_difficulty(work_size);

        assert_eq!(vdf_diff, 7500);
    }

    // Test converting a hex string to bytes
    #[test]
    fn convert_to_bytes() -> Result<(), Box<dyn Error>> {
        let hex_str = "90a9a6923dd9ec246d8046c2bde4d323396f59b2d7a4d43e162db9aeda017b93";
        let result_bytes = to_bytes(hex_str)?;
        let expected_result = [144, 169, 166, 146, 61, 217, 236, 36, 109, 128, 70, 194, 189, 228, 211, 35, 57, 111, 89, 178, 215, 164, 212, 62, 22, 45, 185, 174, 218, 1, 123, 147];
        assert_eq!(result_bytes, expected_result);
        
        Ok(())
    }

    // Get the proper fee from a WU amount
    #[test]
    fn test_fee() {
        let money: WU = WU::try_from(20000).unwrap();
        let fee = WU::try_from(20).unwrap();

        assert_eq!(fee_price(money), fee)
    }
}