//! Material at stake: static exchange evaluation, and how much material a
//! move gives away (to recognise sacrifices).

use chess_core::{Bitboard, Move, MoveKind, PieceKind, Position, Square};

/// Value of a piece, in centipawns (a king is worth more than anything).
#[must_use]
pub const fn value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight | PieceKind::Bishop => 300,
        PieceKind::Rook => 500,
        PieceKind::Queen => 900,
        PieceKind::King => 20_000,
    }
}

/// Material won, in centipawns, by capturing on `target` with the piece on
/// `from`, both sides then recapturing with their least valuable piece as
/// long as it pays (static exchange evaluation). Pins are ignored; a king
/// does not capture a defended piece.
#[must_use]
pub fn exchange(position: &Position, from: Square, target: Square) -> i32 {
    let (Some(attacker), Some(victim)) = (position.piece_at(from), position.piece_at(target))
    else {
        return 0;
    };
    // gain[d]: what the side capturing at depth d wins if the exchange
    // stops after its capture.
    let mut gain = [0_i32; 32];
    gain[0] = value(victim.kind);
    let mut depth = 0;
    let mut occupied = position.occupied().without(from);
    let mut on_target = value(attacker.kind);
    let mut side = !attacker.color;
    while depth + 1 < gain.len() {
        depth += 1;
        gain[depth] = on_target - gain[depth - 1];
        let attackers = position.attackers_to(target, occupied) & occupied;
        let Some((square, kind)) = least_valuable(position, attackers & position.occupied_by(side))
        else {
            break;
        };
        if kind == PieceKind::King && !(attackers & position.occupied_by(!side)).is_empty() {
            break;
        }
        occupied = occupied.without(square);
        on_target = value(kind);
        side = !side;
    }
    // Each side stops the exchange when going on would lose more.
    while depth > 1 {
        depth -= 1;
        gain[depth - 1] = -(-gain[depth - 1]).max(gain[depth]);
    }
    gain[0]
}

/// Material the owner of the piece on `square` would lose there if its
/// opponent could capture now: the exchange started by the least valuable
/// attacker (pins and checks ignored); 0 for a king or an empty square.
#[must_use]
pub fn threat(position: &Position, square: Square) -> i32 {
    let Some(piece) = position.piece_at(square) else {
        return 0;
    };
    if piece.kind == PieceKind::King {
        return 0;
    }
    let occupied = position.occupied();
    let attackers = position.attackers_to(square, occupied) & position.occupied_by(!piece.color);
    let Some((from, kind)) = least_valuable(position, attackers) else {
        return 0;
    };
    if kind == PieceKind::King
        && position.is_attacked_by(square, piece.color, occupied.without(from))
    {
        return 0;
    }
    exchange(position, from, square).max(0)
}

/// Material, in centipawns, that `mv` gives away: what the opponent can
/// then win by capturing the piece moved or, if more, on another piece
/// beyond what it could before (a piece the move uncovers), less what the
/// move captures (or gains by promoting). Positive for a sacrifice.
///
/// The moved piece counts in full even when it was attacked already: an
/// attacked rook taking a bishop where a pawn takes it back gives the
/// exchange away all the same.
#[must_use]
pub fn sacrifice(position: &Position, mv: Move) -> i32 {
    let captured = match mv.kind() {
        MoveKind::EnPassant => value(PieceKind::Pawn),
        _ => position
            .piece_at(mv.to())
            .map_or(0, |piece| value(piece.kind)),
    };
    let promoted = mv
        .promotion()
        .map_or(0, |kind| value(kind) - value(PieceKind::Pawn));
    let mut after = position.without_history();
    after.make_move(mv);
    let given = after
        .occupied_by(position.side_to_move())
        .without(mv.to())
        .map(|square| threat(&after, square) - threat(position, square))
        .fold(threat(&after, mv.to()), i32::max);
    given - captured - promoted
}

/// Least valuable piece among `pieces`, with its square.
fn least_valuable(position: &Position, pieces: Bitboard) -> Option<(Square, PieceKind)> {
    PieceKind::ALL.into_iter().find_map(|kind| {
        (pieces & position.pieces_of_kind(kind))
            .lsb()
            .map(|square| (square, kind))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(fen: &str) -> Position {
        Position::from_fen(fen).unwrap()
    }

    fn square(name: &str) -> Square {
        name.parse().unwrap()
    }

    #[test]
    fn exchanges_stop_when_they_stop_paying() {
        // exd5 exd5: a knight for a pawn.
        let pos = position("4k3/8/4p3/3n4/4P3/8/8/4K3 w - - 0 1");
        assert_eq!(exchange(&pos, square("e4"), square("d5")), 200);
        // Rxd5 exd5: a rook for a knight.
        let pos = position("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1");
        assert_eq!(exchange(&pos, square("d1"), square("d5")), -200);
        // An undefended knight.
        let pos = position("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1");
        assert_eq!(exchange(&pos, square("d1"), square("d5")), 300);
        // Doubled rooks against one defender: the second rook x-rays.
        let pos = position("3rk3/8/8/3p4/8/8/3R4/3RK3 w - - 0 1");
        assert_eq!(exchange(&pos, square("d2"), square("d5")), 100);
        // The king does not take a defended pawn.
        let pos = position("4k3/8/2p5/3p4/4K3/8/8/8 w - - 0 1");
        assert_eq!(threat(&pos, square("d5")), 0);
    }

    #[test]
    fn threatened_material() {
        // The knight on d5 is attacked by the rook and defended by nothing.
        let pos = position("4k3/8/8/3n4/8/8/8/3RK3 b - - 0 1");
        assert_eq!(threat(&pos, square("d5")), 300);
        assert_eq!(threat(&pos, square("d1")), 0);
        let pos = position("4k3/8/4p3/3n4/8/8/8/3RK3 b - - 0 1");
        assert_eq!(threat(&pos, square("d5")), 0);
    }

    #[test]
    fn sacrifices_are_measured() {
        // Qh5 puts the queen where nothing attacks it.
        let pos = position("6k1/5pp1/7p/8/8/8/5PP1/3Q2K1 w - - 0 1");
        let qh5 = pos.parse_uci_move("d1h5").unwrap();
        assert_eq!(sacrifice(&pos, qh5), 0);
        // Qxh6 gxh6: a queen for a pawn.
        let pos = position("6k1/5pp1/7p/7Q/8/8/5PP1/6K1 w - - 0 1");
        let qxh6 = pos.parse_uci_move("h5h6").unwrap();
        assert_eq!(sacrifice(&pos, qxh6), 800);
        // Winning a knight for a pawn gives nothing away.
        let pos = position("4k3/8/4p3/3n4/4P3/8/8/4K3 w - - 0 1");
        let exd5 = pos.parse_uci_move("e4d5").unwrap();
        assert_eq!(sacrifice(&pos, exd5), -200);
        // 15... Rxc4!! 16. bxc4 Bxe3: the rook on d4, attacked by the
        // bishop, gives itself for the other bishop, uncovering the one on
        // e3 to Bc5. The exchange given away, though the rook was attacked.
        let pos = position("r1b3k1/5ppp/2p1p3/p1b5/2BrpP2/PP2B3/2P3PP/R3K2R b KQ - 1 15");
        let rxc4 = pos.parse_uci_move("d4c4").unwrap();
        assert_eq!(sacrifice(&pos, rxc4), 200);
        // Legal's mate: 5. Nxe5 uncovers the queen to the pinning bishop
        // (Bxd1 Kxd1), less the pawn it takes.
        let pos =
            position("rn1qkbnr/ppp2p1p/3p2p1/4p3/2B1P1b1/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 5");
        let nxe5 = pos.parse_uci_move("f3e5").unwrap();
        assert_eq!(sacrifice(&pos, nxe5), 500);
        // Taking the queen while uncovering a knight to a rook gives nothing.
        let pos = position("2r1kb1r/p3qppp/n1N1bn2/8/4p2B/1PN5/P1P2PPP/R2Q1RK1 w k - 3 14");
        let nxe7 = pos.parse_uci_move("c6e7").unwrap();
        assert!(sacrifice(&pos, nxe7) < 0);
    }
}
