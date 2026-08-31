use std::{fmt, format, iter::Sum, ops::{Add, Div, Mul, Sub}, write};

use borsh::{BorshDeserialize, BorshSerialize};

use num_format::{Locale, ToFormattedString};

use super::functions;

// Max whiteroom Size
pub const WHITEROOM_MAX: usize = (3 * 1) + 1;

// VDF constant
pub const VDF_CONSTANT: u128 = 100000;

// Define type of Work address
pub type WorkAddr = ID;

#[derive(Debug)]
pub enum ValueError {
    ExceedsMaximum,
    BelowMinimum,
}

// Hold how coin amount in Work Units type
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct WU(u128);

impl WU {
    pub fn inner(&self) -> u128 {
        self.0
    }

    // WU single
    pub fn single() -> WU {
        WU(1)
    }
    
    pub fn ten_fold(&self) -> Self {
        Self(self.0 * 10)
    }

    // 1 goldbar
    pub fn GDB() -> Self {
        WU(3 * 10u128.pow(15))
    }

    // 1 goldcoin
    pub fn GDC() -> Self {
        WU(3 * 10u128.pow(12))
    }

    // 1 goldcent
    pub fn GDS() -> Self {
        WU(3 * 10u128.pow(9))
    }
}

// The default value of WU is 0
impl Default for WU {
    /// Returns default value 0
    /// 
    /// # Example
    /// 
    /// ```
    /// use marketplace_helpers::objects::WU;
    /// 
    /// let x = WU::default();
    /// 
    /// assert_eq!(x, WU::try_from(0).unwrap());
    /// ```
    
    fn default() -> Self {
        WU(0)
    }
}

impl fmt::Display for WU {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_formatted_string(&Locale::en))
    }
}

impl TryFrom<u128> for WU {
    type Error = ValueError;
    fn try_from(value: u128) -> Result<Self, Self::Error> {
        if value > u128::MAX {
            return Err(ValueError::ExceedsMaximum)
        }

        Ok(Self(value))
    }
}

impl Add for WU {
    type Output = WU;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for WU {
    type Output = WU;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }    
}

impl Mul for WU {
    type Output = WU;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Div for WU {
    type Output = WU;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Sum for WU {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(
            WU::default(),
            |sum, val| sum + val
        )
    }
}

// Job minimum work size
pub const MIN_WORK_SIZE: WU = WU(10000);

// Maximum job work size
pub const MAX_WORK_SIZE: WU = WU(3 * 10u128.pow(15));

// Default type of hash
pub type ID = [u8; 32];

// Trait that satisfies being a whiteroom member/witness
pub trait Member {
    fn id(&self) -> ID;

    fn job_id(&self) -> ID;
}

// Hash trait
pub trait IdHash {
    fn id(&self) -> ID;
}

impl<T> IdHash for T
where 
    T: BorshSerialize,
{
    fn id(&self) -> ID {
        // Serialize message into bytes
        let bytes = borsh::to_vec(&self).expect("Job message serialization failed");

        // Hash serialized bytes and return
        functions::hash(&bytes)
    }
}

pub trait Hash {
    fn hash(&self) -> ID;
}

// Type of work
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Stall {
    pub id: ID,
    pub cost: WU,
}

// Clear error message
pub type AgentResult<T> = Result<T, String>;

/// VRF threshold
pub type VRF_T = ID;

// Work size
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct WorkSize(WU);

impl WorkSize {
    pub fn build(size: WU) -> AgentResult<Self> {
        if size < MIN_WORK_SIZE {
            return Err(format!(
                "Error: Work Size must be greater than or equal to 10000"
            ));
        } else if size > MAX_WORK_SIZE {
            return Err(format!(
                "Error: Work Size must be less than or equal to {}",
                MAX_WORK_SIZE
            ));
        } else {
            Ok(WorkSize(size))
        }
    }

    pub fn into(self) -> WU {
        self.0
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // Trying to create more than max WU
    #[test]
    #[should_panic]
    fn wu_has_max() {
        let num = 2u128.pow(128);

        WU::try_from(num).unwrap();
    }

    // Test Work size
    #[test]
    fn test_work_size() {
        let bad_size = WU::try_from(200).unwrap();
        let good_size = WU::try_from(30000).unwrap();

        assert!(WorkSize::build(bad_size).is_err());
        assert!(WorkSize::build(good_size).is_ok());
    } 
}