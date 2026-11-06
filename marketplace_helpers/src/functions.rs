use std::{todo};

use sha3::{Digest, Sha3_256};
use time::OffsetDateTime;

use crate::objects::{CONTRACT_WHITEROOM_SIZE, ID, MIN_WORK_SIZE, VDF_CONSTANT, VRF_T, WU};
use alloy_primitives::{U256, U512};
use rug::{Rational};

/// BFT whiteroom size given f amounts of tolerable faulty nodes
pub const fn bft_from(f: usize) -> usize {
    (3 * f) + 1
}

/// BFT tolerable number of non-faulty nodes given an
/// expected whiteroom size of exp
pub const fn bft_thresh(exp: usize) -> usize {
    let f = (exp - 1) / 3;

    (2 * f) + 1
}

/// BFT expected whiteroom size given a certain threshold
pub const fn bft_expected_from_thresh(thresh: usize) -> usize {
    // Get number of tolearble faulty nodes
    let f = (thresh - 1) / 2;

    bft_from(f)
}

/// Maximum whiteroom size given by 
/// formula in the marketplace whitepaper.
/// from an expected number of nodes
pub const fn max_wr_size_from_expected(exp: usize) -> usize {
    let threshold = bft_thresh(exp);

    (2 * threshold) - 1
}

/// Expected whiteroom size from  a maximum 
/// whiteroom size max
pub const fn expected_wr_from_max(max: usize) -> usize {
    let thresh = (max + 1) / 2;

    bft_expected_from_thresh(thresh)
}

pub const fn bft_thresh_from_max_wr_size(max: usize) -> usize {
    // Get expected whiteroom size "s" from max
    let s = expected_wr_from_max(max);

    // Return max size (see whitepaper)
    (2 * s) - 1
}

// Get vdf size by multiplying 
// the result of work_size divided by 10000 and
// the result of vrf threshold divided by 2 to 256 power.
pub fn vdf_difficulty(work_size: WU, vrf_threshold: VRF_T) -> u64 {
    // Divide by a value of 100000 to account for
    // difference between one cpu cycle and one VDF squaring
    let work_size = work_size.inner() / VDF_CONSTANT;

    // Convert VRF threshold from bytes to integer
    let vrf_t = U256::from_be_bytes(vrf_threshold);

    // Invested Power (see whitepaper Page 5)
    let ip = work_size / CONTRACT_WHITEROOM_SIZE as u128;

    // Multiply VRF threshold by Invested power, then divide by 
    // max 256 bit number and get final VDF difficulty in u64
    let result = U512::from(vrf_t) * U512::from(ip);

    let bytes = result.to_be_bytes::<64>();

    let vdf_diff = u128::from_be_bytes(bytes[16..32].try_into().unwrap());

    let diff = u64::try_from(vdf_diff).unwrap_or(u64::MAX);

    // The absolute minimum difficulty is 1
    std::cmp::max(diff, 1)

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

/// Get new VRF threshold using whiteroom average and
/// previous VRF threshold as described in the marketplace 
/// whitepaper (Page 4).
/// 
/// wr_avg: Average Whiteroom size in a block 
/// referenced by the block header
/// 
/// prev_vrf: Previous VRF threshold indicating Whiteroom
/// Selction Probability.
pub fn get_vrf(wr_avg_size: usize, prev_vrf: VRF_T) -> VRF_T {
    // Previous VRF threshold and whiteroom average
    // in 256 bit integer form
    let vrf = U256::from_be_bytes(prev_vrf);

    let whiteroom_avg_size = U256::from(wr_avg_size);

    // Expected Whiteroom size in 256 bit integer form.
    let expected_wr_size = U256::from(CONTRACT_WHITEROOM_SIZE);

    ((vrf / whiteroom_avg_size).saturating_mul(expected_wr_size)).to_be_bytes()
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
pub fn wr_seed(work_ptr_id: &[u8], pk: &[u8]) -> ID {
    seed(work_ptr_id, pk)
}

// Get any seed by hashing two inputs
pub fn seed(ipt_0: &[u8], ipt_1: &[u8]) -> ID {
    let mut hasher = Sha3_256::new();
    hasher.update(ipt_0);
    hasher.update(ipt_1);

    hasher.finalize().into()
}

// Hash a series of bytes
pub fn hash(bytes: &[u8]) -> ID {
    Sha3_256::digest(bytes).into()
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

/// Probability of voters chosen 
/// is given by  p = expected_users / total_currency
pub fn voter_probability(users: u128, total_currency: WU) -> f64 {
    let rational = Rational::from(users / total_currency.inner());

    rational.to_f64()
}

/// Hash constant given by 
/// 
/// c = hash / 2 ^ 256
/// where 256 bits is the hash length.
pub fn hashconstant(hash: ID) -> f64 {
    let hash = U256::from_be_bytes(hash);

    let value = (hash >> 203usize).to::<u64>();

    value as f64 / ((1u64 << 53) - 1) as f64
}


#[cfg(test)]
mod tests {
    use std::{assert_eq, error::Error};

    use crate::objects::WU;

    use super::*;

    // Test new VRF threshold formula
    #[test]
    fn test_vrf_threshold() {
        // VRF from previous epoch
        let old_vrf = [255u8; 32];

        // Average whiteroom size of last epoch
        let avg_wr_size = max_wr_size_from_expected(CONTRACT_WHITEROOM_SIZE);

        let new_vrf = get_vrf(avg_wr_size, old_vrf);

        assert_eq!(new_vrf, [204; 32]);
    }

    // Test VRF threshhold cannot overflow max 26 bit number
    #[test]
    fn test_vrf_threshold_overflow() {
        // VRF from previous epoch
        let old_vrf = [255u8; 32];

        // Average whiteroom size of last epoch
        let avg_wr_size = bft_thresh(CONTRACT_WHITEROOM_SIZE);

        let new_vrf = get_vrf(avg_wr_size, old_vrf);

        assert_eq!(new_vrf, [255; 32]);
    }

    // Test vdf difficulty
    #[test]
    fn test_vdf_diff() {
        let work_size = WU::try_from(3000000000).unwrap();

        let vdf_diff = vdf_difficulty(work_size, [255u8; 32]);

        assert_eq!(vdf_diff, 7499);
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

    // BFT helper conversions should preserve the configured quorum relationships.
    #[test]
    fn bft_size_helpers_round_trip_threshold() {
        let expected = 4;
        let threshold = bft_thresh(expected);

        assert_eq!(threshold, 3);
        assert_eq!(bft_expected_from_thresh(threshold), expected);
        assert_eq!(bft_from(1), expected);
        assert_eq!(expected_wr_from_max(max_wr_size_from_expected(expected)), expected);
        assert_eq!(bft_thresh_from_max_wr_size(max_wr_size_from_expected(expected)), 7);
    }

    // Averaging work units should return their arithmetic mean and stay at least one unit.
    #[test]
    fn average_work_units_handles_regular_and_zero_values() {
        assert_eq!(
            avg_wu(WU::try_from(4).unwrap(), WU::try_from(8).unwrap()).inner(),
            6
        );
        assert_eq!(avg_wu(WU::default(), WU::default()), WU::single());
    }

    // Hashing and combining seed components should produce stable 32-byte identifiers.
    #[test]
    fn hash_and_seed_are_deterministic() {
        let first = hash(b"marketplace");
        let second = hash(b"marketplace");

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
        assert_eq!(seed(b"work", b"owner"), seed(b"work", b"owner"));
        assert_ne!(seed(b"work", b"owner"), seed(b"owner", b"work"));
        assert_eq!(wr_seed(b"work", b"owner"), seed(b"work", b"owner"));
    }

    // Address generation should append a checksum derived from the raw 32-byte address.
    #[test]
    fn generated_address_appends_hash_checksum() {
        let raw_address = [23u8; 32];
        let address = generate_address_and_checksum(&raw_address);
        let expected_checksum = hash(&raw_address);

        assert_eq!(address.len(), 36);
        assert_eq!(&address[..32], &raw_address);
        assert_eq!(&address[32..], &expected_checksum[..4]);
    }

    // Byte conversion should reject malformed hex and decode valid fixed-width IDs.
    #[test]
    fn byte_conversion_rejects_invalid_hex() {
        assert!(to_bytes("not-hex").is_err());
        assert!(to_bytes("00").is_err());
        assert_eq!(from_bytes(&dum_bytes()).len(), 64);
    }

    // Hash constants should stay in the normalized interval used by sortition.
    #[test]
    fn hash_constant_is_normalized() {
        assert_eq!(hashconstant([0; 32]), 0.0);
        assert!(hashconstant([255; 32]) <= 1.0);
    }

    // Timestamp generation should return a usable nonnegative Unix timestamp.
    #[test]
    fn timestamp_returns_unix_seconds() {
        assert!(timestamp() > 0);
    }

    // TODO: test that hash constant function should always produce the same f64 value
    // without any bit problems suitable for a decentralized platform
}