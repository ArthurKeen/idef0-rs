//! Conformance profiles (D8, D11; `docs/spec/metamodel.md` §1.2).

use serde::{Deserialize, Serialize};

/// The conformance profile a model is validated under.
///
/// `Extended` is the default and allows up to 7 boxes per non-context diagram
/// **[Deviation]**; `Fips183` keeps the FIPS limit of 6 (§3.3.3-4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// idef0-rs default: extensions on, 3–7 boxes per diagram.
    #[default]
    Extended,
    /// Strict FIPS PUB 183: extensions off or info, 3–6 boxes per diagram.
    Fips183,
}

impl Profile {
    /// The highest box number a non-context diagram may use under this profile.
    ///
    /// Node numbers accept box digits 1–7 in every profile (metamodel §3.2); this limit
    /// is what the box-count rule (`box_count_high`) and [`crate::id::NodeId::fits_profile`]
    /// check against.
    pub const fn max_box_number(self) -> u8 {
        match self {
            Self::Extended => 7,
            Self::Fips183 => 6,
        }
    }
}
