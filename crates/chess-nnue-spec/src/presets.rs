//! Ready-made architectures, stored as the JSON documents users would write
//! (`presets/<id>.json`).

use crate::{Architecture, Format, parse};

/// Id of the preset matching the networks trained before architectures
/// became configurable (`CBNN` version 1 files).
pub const LEGACY: &str = "legacy-768x256";

const SOURCES: [(&str, &str); 4] = [
    ("fast-768", include_str!("../presets/fast-768.json")),
    (
        "halfka-medium",
        include_str!("../presets/halfka-medium.json"),
    ),
    ("multilayer", include_str!("../presets/multilayer.json")),
    (LEGACY, include_str!("../presets/legacy-768x256.json")),
];

/// A named, valid architecture.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// Stable identifier, e.g. `fast-768`.
    pub id: &'static str,
    /// The architecture.
    pub architecture: Architecture,
}

/// Every preset. (They are checked by the tests, so none is ever skipped.)
#[must_use]
pub fn all() -> Vec<Preset> {
    SOURCES
        .iter()
        .filter_map(|&(id, source)| {
            parse(source, Format::Json)
                .ok()
                .map(|architecture| Preset { id, architecture })
        })
        .collect()
}

/// The preset `id`.
#[must_use]
pub fn get(id: &str) -> Option<Preset> {
    all().into_iter().find(|preset| preset.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_valid_and_distinct() {
        for (id, source) in SOURCES {
            if let Err(errors) = parse(source, Format::Json) {
                panic!("preset {id} is invalid: {errors:#?}");
            }
        }
        let presets = all();
        assert_eq!(presets.len(), SOURCES.len());
        let mut hashes: Vec<_> = presets.iter().map(|p| p.architecture.hash()).collect();
        hashes.sort_by_key(|h| h.0);
        hashes.dedup();
        assert_eq!(
            hashes.len(),
            presets.len(),
            "two presets describe the same network"
        );
    }
}
