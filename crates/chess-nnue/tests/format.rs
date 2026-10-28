//! Network files: headers, hashes and every way a file can be rejected.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chess_nnue::spec::presets;
use chess_nnue::{DenseWeights, FORMAT_VERSION, Network, NnueError, inspect};

fn network() -> Network {
    Network::pseudo_random(&presets::get("halfka-medium").unwrap().architecture, 1).unwrap()
}

/// Offset of the architecture JSON in a version 2 file.
const SPEC_AT: usize = 88;

#[test]
fn header_describes_the_network() {
    let network = network();
    let bytes = network.to_bytes();
    assert_eq!(&bytes[..4], b"CBNN");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        FORMAT_VERSION
    );
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(header_len % 64, 0);
    assert_eq!(&bytes[24..56], &network.hash().0);
    let info = inspect(&bytes).unwrap();
    assert_eq!(info.version, 2);
    assert_eq!(info.hash, network.hash());
    assert_eq!(
        info.architecture.metadata.name,
        "HalfKA medium (8 king buckets, 8 output buckets)"
    );
}

#[test]
fn rejects_a_header_whose_hash_does_not_match() {
    let bytes = network().to_bytes();

    // The recorded hash altered.
    let mut wrong_hash = bytes.clone();
    wrong_hash[30] ^= 0xFF;
    assert!(matches!(
        Network::from_bytes(&wrong_hash),
        Err(NnueError::HashMismatch { .. })
    ));

    // The architecture altered (same length, still valid): the weights
    // were not trained for it, and the hash says so.
    let text = String::from_utf8_lossy(&bytes[SPEC_AT..]).into_owned();
    let at = SPEC_AT + text.find("\"evalScale\":400").unwrap() + "\"evalScale\":".len();
    let mut other_arch = bytes.clone();
    other_arch[at..at + 3].copy_from_slice(b"410");
    let err = Network::from_bytes(&other_arch).unwrap_err();
    assert!(matches!(err, NnueError::HashMismatch { .. }), "{err}");
    assert!(err.to_string().contains(&network().hash().to_string()));

    // A network of another architecture than the one asked for.
    let legacy = presets::get(presets::LEGACY).unwrap().architecture;
    assert!(matches!(
        network().expect_hash(legacy.hash()),
        Err(NnueError::HashMismatch { .. })
    ));
    network().expect_hash(network().hash()).unwrap();
}

#[test]
fn rejects_damaged_files() {
    let bytes = network().to_bytes();
    let mut corrupted = bytes.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    assert_eq!(
        Network::from_bytes(&corrupted).unwrap_err(),
        NnueError::Corrupted
    );

    assert!(matches!(
        Network::from_bytes(&bytes[..bytes.len() - 1]),
        Err(NnueError::Length { .. })
    ));
    assert_eq!(
        Network::from_bytes(b"nope").unwrap_err(),
        NnueError::BadMagic
    );
    let mut future = bytes.clone();
    future[4] = 3;
    assert_eq!(
        Network::from_bytes(&future).unwrap_err(),
        NnueError::UnsupportedVersion(3)
    );

    let mut bad_spec = bytes;
    bad_spec[SPEC_AT] = b'[';
    assert!(matches!(
        Network::from_bytes(&bad_spec),
        Err(NnueError::Architecture(_))
    ));
}

#[test]
fn rejects_weights_beyond_the_quantization_bounds() {
    let arch = presets::get("fast-768").unwrap().architecture;
    let layout = arch.layout();
    let zeros = Network::zeros(&arch).unwrap();
    assert_eq!(zeros.evaluate(&chess_core::Position::startpos()), 0);

    let mut ft = vec![0; layout.inputs * layout.accumulator];
    ft[5] = (layout.ft_weight_max + 1) as i16;
    let dense = vec![DenseWeights {
        weights: vec![0; 512],
        bias: vec![0],
    }];
    let err = Network::from_parts(&arch, ft, vec![0; 256], dense.clone()).unwrap_err();
    assert!(matches!(err, NnueError::Range(_)), "{err}");

    let mut wide = dense;
    wide[0].weights[3] = 200;
    let err = Network::from_parts(&arch, vec![0; 768 * 256], vec![0; 256], wide).unwrap_err();
    assert!(matches!(err, NnueError::Range(_)), "{err}");

    let err = Network::from_parts(&arch, vec![0; 10], vec![0; 256], vec![]).unwrap_err();
    assert!(matches!(err, NnueError::Shape(_)), "{err}");
}

#[test]
fn legacy_files_load_with_the_legacy_architecture() {
    let v1 = chess_nnue::legacy::NnueWeights::zeros().to_bytes();
    let info = inspect(&v1).unwrap();
    assert_eq!(info.version, 1);
    let legacy = presets::get(presets::LEGACY).unwrap().architecture;
    assert_eq!(info.hash, legacy.hash());
    assert_eq!(Network::from_bytes(&v1).unwrap().hash(), legacy.hash());
}
