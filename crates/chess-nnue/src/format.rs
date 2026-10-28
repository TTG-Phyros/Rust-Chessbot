//! `CBNN` network files. Version 2 is self-describing (the architecture is
//! in the header); version 1 files are the legacy network, loaded with the
//! `legacy-768x256` architecture. See `docs/network-format.md`.

use chess_nnue_spec::{Architecture, ArchitectureHash, Format, WeightType, presets};
use sha2::{Digest, Sha256};

use crate::legacy::{self, NnueWeights};
use crate::network::{DenseWeights, Network};
use crate::{MAGIC, NnueError};

/// Version written by [`Network::to_bytes`].
pub const FORMAT_VERSION: u32 = 2;

/// Fixed part of the version 2 header, before the architecture JSON.
const FIXED_HEADER: usize = 88;
/// The header is padded to a multiple of this, so tensors start aligned.
const ALIGN: usize = 64;
/// Largest architecture document accepted in a header.
const MAX_SPEC_LEN: usize = 1 << 20;

/// What the header of a network file says, without loading the weights.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Format version (1 or 2).
    pub version: u32,
    /// The architecture (the `legacy-768x256` preset for version 1).
    pub architecture: Architecture,
    /// Architecture hash recorded in the file (computed for version 1).
    pub hash: ArchitectureHash,
    /// Size of the file.
    pub bytes: usize,
}

impl Network {
    /// Loads a network file (version 2, or the legacy version 1).
    ///
    /// # Errors
    ///
    /// [`NnueError`]: not a network, unknown version, invalid architecture,
    /// architecture hash that does not match the architecture, corrupted or
    /// truncated weights, values out of the architecture's bounds.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, NnueError> {
        match version(bytes)? {
            legacy::FORMAT_VERSION => Self::from_legacy(&NnueWeights::from_bytes(bytes)?),
            FORMAT_VERSION => {
                let header = Header::parse(bytes)?;
                let payload = &bytes[header.len..];
                let digest: [u8; 32] = Sha256::digest(payload).into();
                if digest != header.payload_hash {
                    return Err(NnueError::Corrupted);
                }
                decode(&header.architecture, payload)
            }
            other => Err(NnueError::UnsupportedVersion(other)),
        }
    }

    /// Converts version 1 weights (same values, same evaluations).
    ///
    /// # Errors
    ///
    /// [`NnueError::Range`] if the weights exceed the legacy bounds (the
    /// trainer always clipped them within).
    pub fn from_legacy(weights: &NnueWeights) -> Result<Self, NnueError> {
        let architecture = legacy_architecture()?;
        Self::from_parts(
            &architecture,
            weights.ft_weights.clone(),
            weights.ft_bias.clone(),
            vec![DenseWeights {
                weights: weights.out_weights.clone(),
                bias: vec![weights.out_bias],
            }],
        )
    }

    /// Serializes in the current format (version 2).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let weight_type = self.architecture().quantization.weights;
        let mut payload = Vec::new();
        for value in self.ft_weights().iter().chain(self.ft_bias()) {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        for layer in self.dense() {
            for &weight in &layer.weights {
                match weight_type {
                    // In range: checked when the network was built.
                    WeightType::I8 => payload.push(weight as i8 as u8),
                    WeightType::I16 => payload.extend_from_slice(&weight.to_le_bytes()),
                }
            }
            for bias in &layer.bias {
                payload.extend_from_slice(&bias.to_le_bytes());
            }
        }

        let spec = serde_json::to_vec(self.architecture()).unwrap_or_default();
        let header_len = (FIXED_HEADER + spec.len()).next_multiple_of(ALIGN);
        let mut bytes = Vec::with_capacity(header_len + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(header_len as u32).to_le_bytes());
        bytes.extend_from_slice(&(spec.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.hash().0);
        bytes.extend_from_slice(&Sha256::digest(&payload));
        bytes.extend_from_slice(&spec);
        bytes.resize(header_len, 0);
        bytes.extend_from_slice(&payload);
        bytes
    }
}

/// The architecture of version 1 files.
///
/// # Errors
///
/// Never in practice (the preset is checked by the tests).
pub fn legacy_architecture() -> Result<Architecture, NnueError> {
    presets::get(presets::LEGACY)
        .map(|preset| preset.architecture)
        .ok_or_else(|| NnueError::Architecture("missing legacy preset".into()))
}

/// Reads the header of a network file.
///
/// # Errors
///
/// As [`Network::from_bytes`], for the header only.
pub fn inspect(bytes: &[u8]) -> Result<FileInfo, NnueError> {
    let version = version(bytes)?;
    let (architecture, hash) = match version {
        legacy::FORMAT_VERSION => {
            NnueWeights::from_bytes(bytes)?;
            let architecture = legacy_architecture()?;
            let hash = architecture.hash();
            (architecture, hash)
        }
        FORMAT_VERSION => {
            let header = Header::parse(bytes)?;
            (header.architecture, ArchitectureHash(header.arch_hash))
        }
        other => return Err(NnueError::UnsupportedVersion(other)),
    };
    Ok(FileInfo {
        version,
        architecture,
        hash,
        bytes: bytes.len(),
    })
}

fn version(bytes: &[u8]) -> Result<u32, NnueError> {
    if bytes.get(..4) != Some(MAGIC.as_slice()) {
        return Err(NnueError::BadMagic);
    }
    Ok(read_u32(bytes, 4).unwrap_or(0))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn read_hash(bytes: &[u8], at: usize) -> Option<[u8; 32]> {
    bytes.get(at..at + 32)?.try_into().ok()
}

/// A validated version 2 header.
struct Header {
    len: usize,
    architecture: Architecture,
    arch_hash: [u8; 32],
    payload_hash: [u8; 32],
}

impl Header {
    fn parse(bytes: &[u8]) -> Result<Self, NnueError> {
        let truncated = || NnueError::Length {
            actual: bytes.len(),
            expected: FIXED_HEADER,
        };
        let len = read_u32(bytes, 8).ok_or_else(truncated)? as usize;
        let spec_len = read_u32(bytes, 12).ok_or_else(truncated)? as usize;
        let payload_len = read_u64(bytes, 16).ok_or_else(truncated)?;
        let arch_hash = read_hash(bytes, 24).ok_or_else(truncated)?;
        let payload_hash = read_hash(bytes, 56).ok_or_else(truncated)?;
        if spec_len > MAX_SPEC_LEN || len < FIXED_HEADER + spec_len || !len.is_multiple_of(ALIGN) {
            return Err(NnueError::Architecture(format!(
                "inconsistent header: length {len}, architecture length {spec_len}"
            )));
        }
        let expected = (len as u64).saturating_add(payload_len);
        if bytes.len() as u64 != expected {
            return Err(NnueError::Length {
                actual: bytes.len(),
                expected: usize::try_from(expected).unwrap_or(usize::MAX),
            });
        }
        let spec = std::str::from_utf8(&bytes[FIXED_HEADER..FIXED_HEADER + spec_len])
            .map_err(|err| NnueError::Architecture(format!("architecture is not UTF-8: {err}")))?;
        let architecture = chess_nnue_spec::parse(spec, Format::Json).map_err(|errors| {
            NnueError::Architecture(
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        let computed = architecture.hash();
        if computed.0 != arch_hash {
            return Err(NnueError::HashMismatch {
                expected: ArchitectureHash(arch_hash).to_string(),
                found: computed.to_string(),
            });
        }
        Ok(Self {
            len,
            architecture,
            arch_hash,
            payload_hash,
        })
    }
}

/// Builds the network of `architecture` from the tensors of a version 2 file.
fn decode(architecture: &Architecture, payload: &[u8]) -> Result<Network, NnueError> {
    let layout = architecture.layout();
    let weight_type = architecture.quantization.weights;
    let expected = 2 * (layout.inputs * layout.accumulator + layout.accumulator)
        + layout
            .dense
            .iter()
            .map(|d| {
                let neurons = d.buckets * d.outputs;
                neurons * d.inputs * weight_type.bytes() as usize + neurons * 4
            })
            .sum::<usize>();
    if payload.len() != expected {
        return Err(NnueError::Shape(format!(
            "weights take {} bytes, the architecture needs {expected}",
            payload.len()
        )));
    }

    let mut reader = Reader(payload);
    let ft_weights = reader.i16s(layout.inputs * layout.accumulator);
    let ft_bias = reader.i16s(layout.accumulator);
    let dense = layout
        .dense
        .iter()
        .map(|d| {
            let neurons = d.buckets * d.outputs;
            let weights = match weight_type {
                WeightType::I8 => reader.i8s(neurons * d.inputs),
                WeightType::I16 => reader.i16s(neurons * d.inputs),
            };
            DenseWeights {
                weights,
                bias: reader.i32s(neurons),
            }
        })
        .collect();
    Network::from_parts(architecture, ft_weights, ft_bias, dense)
}

/// Sequential little-endian reads (sizes checked beforehand).
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let (head, tail) = self.0.split_at(n.min(self.0.len()));
        self.0 = tail;
        head
    }

    fn i8s(&mut self, n: usize) -> Vec<i16> {
        self.take(n).iter().map(|&b| i16::from(b as i8)).collect()
    }

    fn i16s(&mut self, n: usize) -> Vec<i16> {
        let bytes = self.take(2 * n);
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect()
    }

    fn i32s(&mut self, n: usize) -> Vec<i32> {
        let bytes = self.take(4 * n);
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b))
            .collect()
    }
}
