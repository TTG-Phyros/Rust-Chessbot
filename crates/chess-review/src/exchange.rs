//! Material at stake: static exchange evaluation, and how much material a
//! move gives away (to recognise sacrifices).

use chess_core::{Bitboard, Color, Move, MoveKind, PieceKind, Position, Square};

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

/// Most material the side to move wins by one of its legal captures,
/// exchanges included (0 when no capture wins anything).
#[must_use]
pub fn best_capture(position: &Position) -> i32 {
    position
        .legal_moves()
        .iter()
        .filter(|mv| mv.is_capture() && mv.kind() != MoveKind::EnPassant)
        .map(|mv| exchange(position, mv.from(), mv.to()))
        .fold(0, i32::max)
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

/// Most material `color` would lose if its opponent could capture now: the
/// worst [`threat`] on one of its pieces.
#[must_use]
pub fn hanging(position: &Position, color: Color) -> i32 {
    position
        .occupied_by(color)
        .map(|square| threat(position, square))
        .fold(0, i32::max)
}

/// Material, in centipawns, that `mv` gives away: what the opponent can
/// then win by a capture, less what the move captures (or gains by
/// promoting), less what was already hanging before it. Positive for a
/// sacrifice.
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
    let already = hanging(position, position.side_to_move());
    let mut after = position.without_history();
    after.make_move(mv);
    best_capture(&after) - captured - promoted - already
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
        assert_eq!(hanging(&pos, Color::Black), 0);
    }

    #[test]
    fn hanging_material() {
        // The knight on d5 is attacked by the rook and defended by nothing.
        let pos = position("4k3/8/8/3n4/8/8/8/3RK3 b - - 0 1");
        assert_eq!(hanging(&pos, Color::Black), 300);
        assert_eq!(hanging(&pos, Color::White), 0);
        let pos = position("4k3/8/4p3/3n4/8/8/8/3RK3 b - - 0 1");
        assert_eq!(hanging(&pos, Color::Black), 0);
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
        // A plain trade gives nothing away.
        let pos = position("4k3/8/4p3/3n4/4P3/8/8/4K3 w - - 0 1");
        let exd5 = pos.parse_uci_move("e4d5").unwrap();
        assert_eq!(sacrifice(&pos, exd5), -200);
    }
}
