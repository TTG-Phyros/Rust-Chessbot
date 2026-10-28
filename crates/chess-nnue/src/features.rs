//! Input features of an architecture: which rows of the feature transformer
//! a piece activates, from each perspective.

use chess_core::{Color, Piece, PieceKind, Position, Square};
use chess_nnue_spec::{Architecture, FeatureSet, Mirror};

/// What a perspective's features depend on besides the pieces: the bucket
/// and the mirroring chosen by its own king. A king move that changes it
/// forces a refresh of that perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Context {
    /// First feature of the king bucket.
    base: usize,
    /// Applied to square indices: 56 flips ranks (Black), 7 mirrors files.
    xor: usize,
}

/// Feature encoding of an architecture.
#[derive(Debug, Clone)]
pub(crate) struct Encoder {
    set: FeatureSet,
    /// First feature of each plane, by `[own piece][kind]`; `None` for
    /// pieces without features (kings in `HalfKP`).
    planes: [[Option<usize>; 6]; 2],
    king_buckets: [u8; 64],
    mirror: bool,
    /// Perspectives with an accumulator: both, or White only.
    perspectives: &'static [Color],
}

impl Encoder {
    pub(crate) fn new(arch: &Architecture) -> Self {
        let arch = arch.normalized();
        let mut king_buckets = [0; 64];
        if let Some(map) = &arch.features.king_buckets {
            for (bucket, &id) in king_buckets.iter_mut().zip(map) {
                *bucket = id;
            }
        }
        let set = arch.features.set;
        let mut planes = [[None; 6]; 2];
        for (own, row) in planes.iter_mut().enumerate() {
            for (kind, plane) in row.iter_mut().enumerate() {
                let own = own == 1;
                let king = kind == PieceKind::King.index();
                *plane = match (set, king) {
                    (FeatureSet::Chess768, _) => Some((if own { 0 } else { 6 }) + kind),
                    (FeatureSet::HalfKP, true) => None,
                    (FeatureSet::HalfKAv2Hm, true) => Some(10),
                    (FeatureSet::HalfKP | FeatureSet::HalfKAv2Hm, false) => {
                        Some((if own { 0 } else { 5 }) + kind)
                    }
                }
                .map(|plane| plane * 64);
            }
        }
        Self {
            set,
            planes,
            king_buckets,
            mirror: arch.features.mirror == Some(Mirror::KingFile),
            perspectives: if arch.features.perspective {
                &Color::ALL
            } else {
                &[Color::White]
            },
        }
    }

    pub(crate) fn perspectives(&self) -> &'static [Color] {
        self.perspectives
    }

    /// Context of `perspective` when its own king stands on `king`.
    pub(crate) fn context(&self, perspective: Color, king: Square) -> Context {
        let flip = match perspective {
            Color::White => 0,
            Color::Black => 56,
        };
        let relative = king.index() ^ flip;
        let mirror = if self.mirror && relative % 8 >= 4 {
            7
        } else {
            0
        };
        let bucket = usize::from(self.king_buckets[relative ^ mirror]);
        Context {
            base: bucket * self.set.planes() * 64,
            xor: flip ^ mirror,
        }
    }

    /// Context of `perspective` for architectures without king buckets or
    /// mirroring, where the king's square does not matter.
    pub(crate) fn fixed_context(&self, perspective: Color) -> Context {
        Context {
            base: 0,
            xor: if perspective == Color::White { 0 } else { 56 },
        }
    }

    /// Context of `perspective` in `pos`.
    pub(crate) fn context_in(&self, pos: &Position, perspective: Color) -> Context {
        self.context(perspective, pos.king_square(perspective))
    }

    /// Feature of `piece` on `square` from `perspective` (none for kings in
    /// `HalfKP`).
    pub(crate) fn index(
        &self,
        context: Context,
        perspective: Color,
        piece: Piece,
        square: Square,
    ) -> Option<usize> {
        let own = usize::from(piece.color == perspective);
        let plane = self.planes[own][piece.kind.index()]?;
        Some(context.base + plane + (square.index() ^ context.xor))
    }

    /// Active features of `pieces` from `perspective`.
    pub(crate) fn features<'a>(
        &'a self,
        context: Context,
        perspective: Color,
        pieces: impl Iterator<Item = (Piece, Square)> + 'a,
    ) -> impl Iterator<Item = usize> + 'a {
        pieces.filter_map(move |(piece, square)| self.index(context, perspective, piece, square))
    }
}

/// The pieces of `pos`.
pub(crate) fn pieces(pos: &Position) -> impl Iterator<Item = (Piece, Square)> + '_ {
    pos.occupied()
        .filter_map(|square| pos.piece_at(square).map(|piece| (piece, square)))
}

#[cfg(test)]
mod tests {
    use chess_nnue_spec::presets;

    use super::*;
    use crate::legacy;

    fn encoder(id: &str) -> Encoder {
        Encoder::new(&presets::get(id).unwrap().architecture)
    }

    #[test]
    fn chess768_matches_the_legacy_features() {
        let encoder = encoder(presets::LEGACY);
        let pos = Position::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        )
        .unwrap();
        for perspective in Color::ALL {
            let context = encoder.context_in(&pos, perspective);
            let ours: Vec<usize> = encoder
                .features(context, perspective, pieces(&pos))
                .collect();
            let legacy: Vec<usize> = legacy::active_features(&pos, perspective).collect();
            assert_eq!(ours, legacy);
        }
    }

    #[test]
    fn halfka_mirrors_and_buckets_by_own_king() {
        let encoder = encoder("halfka-medium");
        // Kings on g1/g8: both perspectives mirrored; the same position
        // with colors swapped has the same features from the other side.
        let pos = Position::from_fen("6k1/5ppp/8/8/8/8/1q3PPP/3R2K1 w - - 0 1").unwrap();
        let flipped = pos.flipped();
        let features = |pos: &Position, perspective| {
            let context = encoder.context_in(pos, perspective);
            let mut f: Vec<usize> = encoder
                .features(context, perspective, pieces(pos))
                .collect();
            f.sort_unstable();
            f
        };
        assert_eq!(
            features(&pos, Color::White),
            features(&flipped, Color::Black)
        );
        // g1 mirrored is b1: bucket 1 of the preset's map; 11 planes.
        let king = Piece::new(Color::White, PieceKind::King);
        let context = encoder.context(Color::White, Square::G1);
        assert_eq!(
            encoder.index(context, Color::White, king, Square::G1),
            Some(704 + 10 * 64 + Square::B1.index())
        );
        // f1 is seen as c1: another bucket.
        assert_ne!(encoder.context(Color::White, Square::F1), context);
        assert_eq!(encoder.context(Color::White, Square::B1).base, context.base);
    }

    #[test]
    fn halfkp_has_no_king_features() {
        let arch = chess_nnue_spec::parse(
            r#"{"schemaVersion":1,"metadata":{"name":"kp"},"features":{"set":"HalfKP"},
                "accumulator":{"size":16,"activation":"CReLU"},
                "quantization":{"weights":"i8","qa":255,"qb":64,"evalScale":400}}"#,
            chess_nnue_spec::Format::Json,
        )
        .unwrap();
        let encoder = Encoder::new(&arch);
        let pos = Position::startpos();
        let context = encoder.context_in(&pos, Color::Black);
        assert_eq!(
            encoder
                .features(context, Color::Black, pieces(&pos))
                .count(),
            30
        );
        // Black's king on e8 is e1 from its side: bucket 4.
        assert_eq!(context.base, 4 * 640);
    }
}
