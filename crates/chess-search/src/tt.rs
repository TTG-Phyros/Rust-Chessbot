//! Transposition table: a hash map from position to search result, of fixed
//! size, where newer or deeper results replace older ones.

use chess_core::Move;

/// How the stored score relates to the true score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Bound {
    /// No entry.
    #[default]
    None,
    /// The true score is this one.
    Exact,
    /// The true score is at least this (fail high).
    Lower,
    /// The true score is at most this (fail low).
    Upper,
}

/// One stored result (16 bytes).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Entry {
    key: u64,
    pub(crate) best_move: Move,
    pub(crate) score: i16,
    pub(crate) depth: i8,
    pub(crate) bound: Bound,
    age: u8,
}

/// Fixed-size transposition table.
#[derive(Debug)]
pub(crate) struct TranspositionTable {
    entries: Vec<Entry>,
    age: u8,
}

/// Default size in MiB.
pub const DEFAULT_HASH_MB: usize = 16;
/// Largest size accepted, in MiB.
pub const MAX_HASH_MB: usize = 32 * 1024;

impl TranspositionTable {
    pub(crate) fn new(size_mb: usize) -> Self {
        let size = size_mb.clamp(1, MAX_HASH_MB) * 1024 * 1024;
        let len = (size / size_of::<Entry>()).max(1);
        Self {
            entries: vec![Entry::default(); len],
            age: 0,
        }
    }

    /// Forgets everything (new game).
    pub(crate) fn clear(&mut self) {
        self.entries.fill(Entry::default());
        self.age = 0;
    }

    /// Marks the start of a new search: older entries become replaceable.
    pub(crate) fn new_search(&mut self) {
        self.age = self.age.wrapping_add(1);
    }

    fn index(&self, key: u64) -> usize {
        // Maps the key uniformly onto 0..len without a division.
        ((u128::from(key) * self.entries.len() as u128) >> 64) as usize
    }

    pub(crate) fn probe(&self, key: u64) -> Option<Entry> {
        self.entries
            .get(self.index(key))
            .filter(|entry| entry.key == key && entry.bound != Bound::None)
            .copied()
    }

    pub(crate) fn store(
        &mut self,
        key: u64,
        best_move: Move,
        score: i32,
        depth: i32,
        bound: Bound,
    ) {
        let age = self.age;
        let index = self.index(key);
        let Some(slot) = self.entries.get_mut(index) else {
            return;
        };
        let depth = depth.clamp(-1, i32::from(i8::MAX)) as i8;
        let replace = slot.key != key
            || slot.age != age
            || bound == Bound::Exact
            || depth >= slot.depth.saturating_sub(2);
        if !replace {
            return;
        }
        // Keep the old best move if the new search did not find one.
        let best_move = if best_move.is_null() && slot.key == key {
            slot.best_move
        } else {
            best_move
        };
        *slot = Entry {
            key,
            best_move,
            score: score.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
            depth,
            bound,
            age,
        };
    }

    /// Permille of the table used by the current search (UCI `hashfull`).
    pub(crate) fn hashfull(&self) -> u32 {
        let sample = self.entries.len().min(1000);
        let used = self.entries[..sample]
            .iter()
            .filter(|entry| entry.bound != Bound::None && entry.age == self.age)
            .count();
        (used * 1000 / sample.max(1)) as u32
    }
}

#[cfg(test)]
mod tests {
    use chess_core::{MoveKind, Square};

    use super::*;

    #[test]
    fn entry_is_16_bytes() {
        assert_eq!(size_of::<Entry>(), 16);
    }

    #[test]
    fn store_and_probe() {
        let mut tt = TranspositionTable::new(1);
        let mv = Move::new(Square::E2, Square::E4, MoveKind::DoublePawnPush);
        assert!(tt.probe(42).is_none());
        tt.store(42, mv, 35, 6, Bound::Exact);
        let entry = tt.probe(42).unwrap();
        assert_eq!(
            (entry.best_move, entry.score, entry.depth, entry.bound),
            (mv, 35, 6, Bound::Exact)
        );
        assert!(tt.probe(43).is_none());
        tt.clear();
        assert!(tt.probe(42).is_none());
    }

    #[test]
    fn shallow_results_do_not_overwrite_deep_ones_of_the_same_search() {
        let mut tt = TranspositionTable::new(1);
        tt.store(7, Move::NULL, 10, 10, Bound::Lower);
        tt.store(7, Move::NULL, 20, 2, Bound::Lower);
        assert_eq!(tt.probe(7).unwrap().depth, 10);
        tt.new_search();
        tt.store(7, Move::NULL, 20, 2, Bound::Lower);
        assert_eq!(tt.probe(7).unwrap().depth, 2);
    }

    #[test]
    fn hashfull_counts_current_entries() {
        let mut tt = TranspositionTable::new(1);
        assert_eq!(tt.hashfull(), 0);
        for key in 0..100_000_u64 {
            tt.store(
                key.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                Move::NULL,
                0,
                1,
                Bound::Exact,
            );
        }
        assert!(tt.hashfull() > 0);
        tt.new_search();
        assert_eq!(tt.hashfull(), 0);
    }
}
