//! # Marketplace Helpers
//! 
//! This library defines useful and repetitive types and functions found all around
//! the marketplace ecosystem.
pub mod functions;
pub mod objects;

#[cfg(test)]
mod tests {
    use super::{functions, objects::WU};

    // The crate root should expose its shared object types and helper functions.
    #[test]
    fn public_helper_modules_are_available() {
        assert_eq!(WU::GDC().inner(), 3_000_000_000_000);
        assert_eq!(functions::hash(&[0; 32]).len(), 32);
    }
}
