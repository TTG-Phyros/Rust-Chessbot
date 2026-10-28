//! Inference of every kind of architecture: an independent scalar
//! implementation of the documented semantics, incremental updates against
//! full refreshes, color symmetry, and file round trips.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chess_core::{Color, Move, PieceKind, Position, Square};
use chess_nnue::Network;
use chess_nnue::spec::{
    Activation, Architecture, BucketStrategy, FeatureSet, Format, Mirror, parse, presets,
};

const FENS: [&str; 8] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
    "8/8/1k6/2b5/2pP4/8/5K2/8 b - - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "6k1/5ppp/8/8/8/8/1q3PPP/3R2K1 w - - 0 1",
    "4k3/1P6/8/8/8/8/6p1/4K2R w K - 0 1",
];

fn arch(json: &str) -> Architecture {
    parse(json, Format::Json).unwrap_or_else(|e| panic!("{json}: {e:#?}"))
}

/// Presets plus architectures covering what they do not: HalfKP, no
/// perspective, ReLU with i64 sums, SCReLU and ReLU hidden layers, i16
/// dense weights, material buckets on the output layer only.
fn architectures() -> Vec<(String, Architecture)> {
    let mut all: Vec<(String, Architecture)> = presets::all()
        .into_iter()
        .map(|p| (p.id.to_owned(), p.architecture))
        .collect();
    let q = r#""quantization":{"weights":"i8","qa":255,"qb":64,"evalScale":400}"#;
    all.push((
        "halfkp".into(),
        arch(&format!(
            r#"{{"schemaVersion":1,"metadata":{{"name":"kp"}},"features":{{"set":"HalfKP","mirror":"kingFile","kingBuckets":{}}},
               "accumulator":{{"size":32,"activation":"SCReLU"}},
               "hiddenLayers":[{{"size":8,"activation":"SCReLU"}}],{q}}}"#,
            serde_json::to_string(&(0..64).map(|s: u8| (s / 8).min(3) * 4 + (s % 8).min(7 - s % 8)).collect::<Vec<u8>>()).unwrap()
        )),
    ));
    all.push((
        "no-perspective".into(),
        arch(&format!(
            r#"{{"schemaVersion":1,"metadata":{{"name":"np"}},"features":{{"set":"Chess768","perspective":false}},
               "accumulator":{{"size":48,"activation":"CReLU"}},
               "outputBuckets":{{"strategy":"material","count":5}},{q}}}"#
        )),
    ));
    all.push((
        "relu-i64".into(),
        arch(
            r#"{"schemaVersion":1,"metadata":{"name":"relu"},"features":{"set":"Chess768"},
               "accumulator":{"size":64,"activation":"ReLU"},
               "hiddenLayers":[{"size":5,"activation":"ReLU"},{"size":3,"activation":"CReLU"}],
               "outputBuckets":{"strategy":"pieceCount","count":4,"scope":"all"},
               "quantization":{"weights":"i16","qa":181,"qb":512,"evalScale":300,"weightClip":40.0}}"#,
        ),
    ));
    all
}

/// The documented semantics, written independently of the crate: features
/// recomputed from the position, every sum in i64.
fn reference_eval(arch: &Architecture, network: &Network, pos: &Position) -> i32 {
    let arch = arch.normalized();
    let layout = arch.layout();
    let bytes = network.to_bytes();
    // Tensors from the file, the way a third-party reader would get them.
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut payload = &bytes[header_len..];
    let mut i16s = |n: usize| -> Vec<i64> {
        let (head, tail) = payload.split_at(2 * n);
        payload = tail;
        head.chunks(2)
            .map(|b| i64::from(i16::from_le_bytes([b[0], b[1]])))
            .collect()
    };
    let n = layout.accumulator;
    let ft_w = i16s(layout.inputs * n);
    let ft_b = i16s(n);
    let mut dense = Vec::new();
    for d in &layout.dense {
        let count = d.buckets * d.outputs * d.inputs;
        let w: Vec<i64> = match arch.quantization.weights {
            chess_nnue::spec::WeightType::I8 => {
                let (head, tail) = payload.split_at(count);
                payload = tail;
                head.iter().map(|&b| i64::from(b as i8)).collect()
            }
            chess_nnue::spec::WeightType::I16 => {
                let (head, tail) = payload.split_at(2 * count);
                payload = tail;
                head.chunks(2)
                    .map(|b| i64::from(i16::from_le_bytes([b[0], b[1]])))
                    .collect()
            }
        };
        let (head, tail) = payload.split_at(4 * d.buckets * d.outputs);
        payload = tail;
        let b: Vec<i64> = head
            .chunks(4)
            .map(|b| i64::from(i32::from_le_bytes(b.try_into().unwrap())))
            .collect();
        dense.push((w, b));
    }

    let q = &arch.quantization;
    let (qa, qb) = (i64::from(q.qa), i64::from(q.qb));
    let map = arch.features.king_buckets.clone().unwrap();
    let mirror = arch.features.mirror == Some(Mirror::KingFile);
    let planes = arch.features.set.planes();
    let accumulate = |p: Color| -> Vec<i64> {
        let flip = if p == Color::White { 0 } else { 56 };
        let king = pos.king_square(p).index() ^ flip;
        let m = if mirror && king % 8 >= 4 { 7 } else { 0 };
        let bucket = usize::from(map[king ^ m]);
        let mut acc = ft_b.clone();
        for square in pos.occupied() {
            let piece = pos.piece_at(square).unwrap();
            let own = piece.color == p;
            let kind = piece.kind.index();
            let plane = match (arch.features.set, piece.kind) {
                (FeatureSet::Chess768, _) => {
                    if own {
                        kind
                    } else {
                        6 + kind
                    }
                }
                (FeatureSet::HalfKP, PieceKind::King) => continue,
                (FeatureSet::HalfKAv2Hm, PieceKind::King) => 10,
                _ => {
                    if own {
                        kind
                    } else {
                        5 + kind
                    }
                }
            };
            let feature = (bucket * planes + plane) * 64 + (square.index() ^ flip ^ m);
            for (a, w) in acc.iter_mut().zip(&ft_w[feature * n..(feature + 1) * n]) {
                *a += w;
            }
        }
        acc
    };
    let act = |x: i64, a: Activation| -> i64 {
        match a {
            Activation::ReLU => x.clamp(0, 32767),
            Activation::CReLU | Activation::PairwiseCReLU => x.clamp(0, qa),
            Activation::SCReLU => x.clamp(0, qa).pow(2) / qa,
        }
    };
    let stm = pos.side_to_move();
    let sides: Vec<Color> = if arch.features.perspective {
        vec![stm, !stm]
    } else {
        vec![Color::White]
    };
    let mut input = Vec::new();
    for side in sides {
        let acc = accumulate(side);
        match arch.accumulator.activation {
            Activation::PairwiseCReLU => {
                for i in 0..n / 2 {
                    input.push(acc[i].clamp(0, qa) * acc[i + n / 2].clamp(0, qa) / qa);
                }
            }
            a => input.extend(acc.iter().map(|&x| act(x, a))),
        }
    }
    let count = layout.output_buckets;
    let bucket = match arch.output_buckets.strategy {
        BucketStrategy::None => 0,
        BucketStrategy::PieceCount => {
            ((pos.occupied().count() as usize - 2) / (32 / count)).min(count - 1)
        }
        BucketStrategy::Material => {
            let m: usize = [
                (PieceKind::Pawn, 1),
                (PieceKind::Knight, 3),
                (PieceKind::Bishop, 3),
                (PieceKind::Rook, 5),
                (PieceKind::Queen, 9),
            ]
            .iter()
            .map(|&(k, v)| pos.pieces_of_kind(k).count() as usize * v)
            .sum();
            (m * count / 79).min(count - 1)
        }
    };
    for (d, (w, b)) in layout.dense.iter().zip(&dense) {
        let bk = if d.buckets > 1 { bucket } else { 0 };
        let mut out = Vec::new();
        for o in 0..d.outputs {
            let neuron = bk * d.outputs + o;
            let sum: i64 = b[neuron]
                + input
                    .iter()
                    .zip(&w[neuron * d.inputs..(neuron + 1) * d.inputs])
                    .map(|(x, w)| x * w)
                    .sum::<i64>();
            match d.activation {
                None => {
                    let cp = (sum * i64::from(q.eval_scale) / (qa * qb)) as i32;
                    return if !arch.features.perspective && stm == Color::Black {
                        -cp
                    } else {
                        cp
                    };
                }
                Some(a) => out.push(act(sum / qb, a)),
            }
        }
        input = out;
    }
    unreachable!()
}

/// Walks a deterministic tree (every move near the root, a random third
/// deeper) and checks incremental evaluations against full ones.
fn walk(
    network: &Network,
    state: &mut chess_nnue::NnueState,
    pos: &mut Position,
    depth: u32,
    seed: &mut u64,
) {
    assert_eq!(
        network.evaluate_incremental(state, pos),
        network.evaluate(pos),
        "{}",
        pos.to_fen()
    );
    if depth == 0 {
        return;
    }
    let moves = pos.legal_moves();
    for (i, &mv) in moves.iter().enumerate() {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        if depth < 3 && !(*seed >> 33).is_multiple_of(3) && i > 0 {
            continue;
        }
        network.on_make(state, pos, mv);
        pos.make_move(mv);
        walk(network, state, pos, depth - 1, seed);
        pos.unmake_move();
        network.on_unmake(state);
    }
    if !pos.in_check() {
        network.on_make(state, pos, Move::NULL);
        pos.make_null_move();
        assert_eq!(
            network.evaluate_incremental(state, pos),
            network.evaluate(pos)
        );
        pos.unmake_move();
        network.on_unmake(state);
    }
}

#[test]
fn evaluations_follow_the_documented_semantics() {
    for (name, arch) in architectures() {
        let network = Network::pseudo_random(&arch, 99).unwrap();
        for fen in FENS {
            let pos = Position::from_fen(fen).unwrap();
            let expected = reference_eval(&arch, &network, &pos);
            assert_eq!(network.evaluate(&pos), expected, "{name}: {fen}");
            assert_eq!(
                network.clone().generic().evaluate(&pos),
                expected,
                "{name} generic: {fen}"
            );
        }
    }
}

#[test]
fn incremental_updates_match_full_refresh() {
    for (name, arch) in architectures() {
        let network = Network::pseudo_random(&arch, 5).unwrap();
        for fen in FENS {
            let mut pos = Position::from_fen(fen).unwrap();
            let mut state = network.new_state(&pos);
            let depth = if arch.layout().inputs > 10_000 { 2 } else { 3 };
            walk(&network, &mut state, &mut pos, depth, &mut 42);
            assert_eq!(state.ply(), 0, "{name}: unbalanced stack after {fen}");
        }
    }
}

#[test]
fn evaluation_is_symmetric_with_perspective() {
    for (name, arch) in architectures() {
        if !arch.features.perspective {
            continue;
        }
        let network = Network::pseudo_random(&arch, 11).unwrap();
        for fen in FENS {
            let pos = Position::from_fen(fen).unwrap();
            assert_eq!(
                network.evaluate(&pos),
                network.evaluate(&pos.flipped()),
                "{name}: {fen}"
            );
        }
    }
}

#[test]
fn files_round_trip() {
    for (name, arch) in architectures() {
        let network = Network::pseudo_random(&arch, 3).unwrap();
        let bytes = network.to_bytes();
        let back = Network::from_bytes(&bytes).unwrap();
        assert_eq!(back.to_bytes(), bytes, "{name}");
        assert_eq!(back.hash(), arch.hash());
        assert_eq!(back.architecture().metadata, arch.metadata);
        let estimate = arch.estimate().weights_bytes as usize;
        assert!(
            bytes.len() > estimate && bytes.len() < estimate + 4096,
            "{name}"
        );
        let pos = Position::from_fen(FENS[1]).unwrap();
        assert_eq!(back.evaluate(&pos), network.evaluate(&pos));
    }
}

#[test]
fn piece_contribution_is_the_evaluation_without_the_piece() {
    let arch = presets::get("fast-768").unwrap().architecture;
    let network = Network::pseudo_random(&arch, 17).unwrap();
    let pos = Position::from_fen(FENS[1]).unwrap();
    // The knight on e5, removed from the FEN.
    let without =
        Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
            .unwrap();
    let contributions = network.piece_contributions(&pos);
    assert_eq!(contributions.len(), 32);
    let (_, _, cp) = contributions
        .iter()
        .find(|(s, _, _)| *s == Square::E5)
        .copied()
        .unwrap();
    assert_eq!(cp, network.evaluate(&pos) - network.evaluate(&without));
}
