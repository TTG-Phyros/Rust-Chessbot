//! Architecture hash: SHA-256 of the normalized architecture without its
//! metadata, serialized as canonical JSON (RFC 8785). Two documents that
//! describe the same network (defaults written out or not, keys in any
//! order, different names) have the same hash; any change to the weights'
//! layout or meaning changes it.

use sha2::{Digest, Sha256};

use crate::model::Architecture;

/// SHA-256 identifying an architecture; shown as 64 lowercase hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchitectureHash(pub [u8; 32]);

impl std::fmt::Display for ArchitectureHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl std::str::FromStr for ArchitectureHash {
    type Err = String;

    fn from_str(hex: &str) -> Result<Self, String> {
        let invalid = || format!("not a SHA-256 in hex: {hex:?}");
        if hex.len() != 64 || !hex.is_ascii() {
            return Err(invalid());
        }
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
            let pair = std::str::from_utf8(pair).map_err(|_| invalid())?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
        }
        Ok(Self(bytes))
    }
}

impl Architecture {
    /// The canonical JSON the hash is computed on.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        let mut value = serde_json::to_value(self.normalized()).unwrap_or_default();
        if let Some(object) = value.as_object_mut() {
            object.remove("metadata");
        }
        serde_json_canonicalizer::to_string(&value).unwrap_or_default()
    }

    /// Hash of the architecture (metadata excluded).
    #[must_use]
    pub fn hash(&self) -> ArchitectureHash {
        ArchitectureHash(Sha256::digest(self.canonical_json()).into())
    }
}
