//! Perft: counts the leaf nodes of the legal move tree, to validate move
//! generation against known reference counts.

use crate::{Move, Position};

/// Number of leaf nodes `depth` plies below `pos`.
///
/// Uses bulk counting: at depth 1 the number of legal moves is returned
/// without playing them.
pub fn perft(pos: &mut Position, depth: u32) -> u64 {
    match depth {
        0 => 1,
        1 => pos.legal_moves().len() as u64,
        _ => {
            let mut nodes = 0;
            for mv in pos.legal_moves().iter().copied() {
                pos.make_move(mv);
                nodes += perft(pos, depth - 1);
                pos.unmake_move();
            }
            nodes
        }
    }
}

/// Perft split by root move, the usual tool to find a movegen bug by
/// comparing with another engine. `depth` must be at least 1.
pub fn perft_divide(pos: &mut Position, depth: u32) -> Vec<(Move, u64)> {
    let depth = depth.max(1);
    pos.legal_moves()
        .iter()
        .copied()
        .map(|mv| {
            pos.make_move(mv);
            let nodes = perft(pos, depth - 1);
            pos.unmake_move();
            (mv, nodes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shallow_startpos() {
        let mut pos = Position::startpos();
        assert_eq!(perft(&mut pos, 0), 1);
        assert_eq!(perft(&mut pos, 1), 20);
        assert_eq!(perft(&mut pos, 2), 400);
        assert_eq!(perft(&mut pos, 3), 8_902);
    }

    #[test]
    fn divide_sums_to_perft() {
        let mut pos = Position::startpos();
        let divide = perft_divide(&mut pos, 3);
        assert_eq!(divide.len(), 20);
        assert_eq!(divide.iter().map(|(_, n)| n).sum::<u64>(), 8_902);
        assert_eq!(pos, Position::startpos());
    }
}
