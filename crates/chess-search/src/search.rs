//! Negamax alpha-beta (principal variation search) with iterative
//! deepening, aspiration windows, transposition table, quiescence search,
//! null-move pruning, reverse futility pruning and late move reductions.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use chess_core::{Move, Position};
use chess_eval::Evaluator;

use crate::StopFlag;
use crate::limits::{SearchLimits, TimeBudget};
use crate::ordering::{History, Killers, MovePicker, is_noisy};
use crate::score::{DRAW, INFINITY, MATE, MATE_BOUND, MAX_PLY, Score, from_tt, to_tt};
use crate::tt::{Bound, DEFAULT_HASH_MB, TranspositionTable};

/// One of the best lines of a multi-PV search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvLine {
    /// Score for the side to move.
    pub score: Score,
    /// The line, starting with its root move.
    pub pv: Vec<Move>,
}

/// Progress report, emitted after each completed iteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInfo {
    /// Completed depth (plies).
    pub depth: u32,
    /// Deepest ply reached, quiescence included.
    pub seldepth: u32,
    /// Score for the side to move.
    pub score: Score,
    /// Nodes visited so far.
    pub nodes: u64,
    /// Time elapsed since the search started.
    pub time: Duration,
    /// Nodes per second.
    pub nps: u64,
    /// Transposition table usage, in permille.
    pub hashfull: u32,
    /// Principal variation, starting with the best move.
    pub pv: Vec<Move>,
    /// The best lines, best first (`SearchLimits::multipv` of them, or
    /// fewer when there are fewer legal moves); the first is `score` and `pv`.
    pub lines: Vec<PvLine>,
}

/// Outcome of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Best move found; `None` only if the side to move has no legal move.
    pub best_move: Option<Move>,
    /// Expected reply, from the principal variation.
    pub ponder: Option<Move>,
    /// Score of the last completed iteration.
    pub score: Score,
    /// Last completed depth.
    pub depth: u32,
    /// Total nodes visited.
    pub nodes: u64,
    /// Principal variation of the last completed iteration.
    pub pv: Vec<Move>,
    /// The best lines of the last completed iteration, best first.
    pub lines: Vec<PvLine>,
}

/// A searcher bound to an injected evaluator. Keeps its transposition table
/// and history between searches; call [`Searcher::clear`] for a new game.
pub struct Searcher<E> {
    evaluator: E,
    stop: StopFlag,
    tt: TranspositionTable,
    history: History,
}

impl<E: Evaluator> Searcher<E> {
    /// Creates a searcher using `evaluator`, cancellable through `stop`, with
    /// a transposition table of the default size.
    pub fn new(evaluator: E, stop: StopFlag) -> Self {
        Self {
            evaluator,
            stop,
            tt: TranspositionTable::new(DEFAULT_HASH_MB),
            history: History::new(),
        }
    }

    /// Resizes (and clears) the transposition table, in MiB.
    pub fn set_hash_size(&mut self, megabytes: usize) {
        self.tt = TranspositionTable::new(megabytes);
    }

    /// Forgets everything learned in previous searches (new game).
    pub fn clear(&mut self) {
        self.tt.clear();
        self.history.clear();
    }

    /// The flag that stops this searcher.
    pub fn stop_flag(&self) -> &StopFlag {
        &self.stop
    }

    /// The injected evaluator.
    pub fn evaluator(&self) -> &E {
        &self.evaluator
    }

    /// Replaces the evaluator (e.g. a new network) and forgets what was
    /// learned with the previous one.
    pub fn set_evaluator(&mut self, evaluator: E) {
        self.evaluator = evaluator;
        self.clear();
    }

    /// Searches `pos` within `limits`, calling `on_info` after each completed
    /// iteration. The move history of `pos` is used to detect repetitions.
    ///
    /// The stop flag is not reset here: clear it before starting a search.
    pub fn search(
        &mut self,
        pos: &Position,
        limits: &SearchLimits,
        mut on_info: impl FnMut(&SearchInfo),
    ) -> SearchResult {
        self.tt.new_search();
        self.history.age();
        let start = Instant::now();
        let budget = TimeBudget::new(limits, pos.side_to_move());
        let root_moves = pos.legal_moves();

        let mut ctx = Context {
            evaluator: &self.evaluator,
            stop: &self.stop,
            tt: &mut self.tt,
            history: &mut self.history,
            pos: pos.clone(),
            eval_state: self.evaluator.new_state(pos),
            killers: Killers::new(),
            pv: Box::new([[Move::NULL; MAX_PLY + 1]; MAX_PLY + 1]),
            pv_len: [0; MAX_PLY + 1],
            nodes: 0,
            seldepth: 0,
            hard_deadline: budget.hard.map(|hard| start + hard),
            node_limit: limits.nodes,
            stopped: false,
            excluded_root: Vec::new(),
        };

        let mut result = SearchResult {
            best_move: root_moves.first().copied(),
            ponder: None,
            score: Score::from_internal(if pos.in_check() { -MATE } else { DRAW }),
            depth: 0,
            nodes: 0,
            pv: Vec::new(),
            lines: Vec::new(),
        };
        if root_moves.is_empty() {
            return result;
        }
        let multipv = limits.multipv.clamp(1, root_moves.len());

        let max_depth = limits
            .depth
            .unwrap_or(MAX_PLY as u32 - 1)
            .clamp(1, MAX_PLY as u32 - 1);
        // Score of each line at the previous depth, for the aspiration windows.
        let mut previous = vec![0; multipv];
        for depth in 1..=max_depth {
            // Multi-PV: search the root again for each line, without the
            // root moves of the lines already found at this depth.
            ctx.excluded_root.clear();
            let mut lines: Vec<(i32, Vec<Move>)> = Vec::with_capacity(multipv);
            for k in 0..multipv {
                let score = ctx.aspiration(depth as i32, previous.get(k).copied().unwrap_or(0));
                // A partial iteration is only trusted for the very first
                // depth and line, and only if it got as far as a best move.
                let usable = !ctx.stopped || (depth == 1 && k == 0 && ctx.pv_len[0] > 0);
                let pv = ctx.pv[0][..ctx.pv_len[0]].to_vec();
                let Some(&first) = pv.first().filter(|_| usable) else {
                    break;
                };
                ctx.excluded_root.push(first);
                lines.push((score, pv));
                if ctx.stopped {
                    break;
                }
            }
            if lines.is_empty() || (ctx.stopped && depth > 1) {
                break;
            }
            // Later lines may score better than earlier ones (another window).
            lines.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
            previous = lines.iter().map(|(score, _)| *score).collect();
            let (score, pv) = lines[0].clone();
            result.best_move = pv.first().copied();
            result.ponder = pv.get(1).copied();
            result.score = Score::from_internal(score);
            result.depth = depth;
            result.pv.clone_from(&pv);
            result.lines = lines
                .into_iter()
                .map(|(score, pv)| PvLine {
                    score: Score::from_internal(score),
                    pv,
                })
                .collect();

            let time = start.elapsed();
            on_info(&SearchInfo {
                depth,
                seldepth: ctx.seldepth as u32,
                score: result.score,
                nodes: ctx.nodes,
                time,
                nps: (ctx.nodes as f64 / time.as_secs_f64().max(1e-6)) as u64,
                hashfull: ctx.tt.hashfull(),
                pv,
                lines: result.lines.clone(),
            });

            let out_of_time = budget.soft.is_some_and(|soft| time >= soft);
            let forced = root_moves.len() == 1 && limits.is_timed();
            if ctx.stopped || out_of_time || forced {
                break;
            }
        }
        result.nodes = ctx.nodes;
        result
    }
}

/// Reduction for the `moves`-th move at `depth` (logarithmic, as in most engines).
static LMR: LazyLock<[[i32; 64]; 64]> = LazyLock::new(|| {
    let mut table = [[0; 64]; 64];
    for (depth, row) in table.iter_mut().enumerate().skip(1) {
        for (moves, reduction) in row.iter_mut().enumerate().skip(1) {
            *reduction = (0.75 + (depth as f64).ln() * (moves as f64).ln() / 2.25) as i32;
        }
    }
    table
});

/// How often (in nodes) the clock and the stop flag are checked.
const CHECK_INTERVAL: u64 = 1024;

/// State of one search.
struct Context<'a, E: Evaluator> {
    evaluator: &'a E,
    stop: &'a StopFlag,
    tt: &'a mut TranspositionTable,
    history: &'a mut History,
    pos: Position,
    /// Incremental evaluation state (NNUE accumulators), kept in step with `pos`.
    eval_state: E::State,
    killers: Killers,
    /// Triangular PV table: `pv[ply][ply..pv_len[ply]]` is the line from `ply`.
    pv: Box<[[Move; MAX_PLY + 1]; MAX_PLY + 1]>,
    pv_len: [usize; MAX_PLY + 1],
    nodes: u64,
    seldepth: usize,
    hard_deadline: Option<Instant>,
    node_limit: Option<u64>,
    stopped: bool,
    /// Root moves left out of this search (lines already found, multi-PV).
    excluded_root: Vec<Move>,
}

impl<E: Evaluator> Context<'_, E> {
    /// Searches the root with a window around the previous score, widening
    /// it on failure.
    fn aspiration(&mut self, depth: i32, previous: i32) -> i32 {
        let mut delta = 25;
        let (mut alpha, mut beta) = if depth >= 4 && previous.abs() < MATE_BOUND {
            (previous - delta, previous + delta)
        } else {
            (-INFINITY, INFINITY)
        };
        loop {
            let score = self.negamax(depth, alpha, beta, 0, true);
            if self.stopped {
                return score;
            }
            if score <= alpha {
                beta = (alpha + beta) / 2;
                alpha = (score - delta).max(-INFINITY);
            } else if score >= beta {
                beta = (score + delta).min(INFINITY);
            } else {
                return score;
            }
            delta *= 2;
            if delta > 1000 {
                alpha = -INFINITY;
                beta = INFINITY;
            }
        }
    }

    /// Checks the node limit every node, the clock and stop flag periodically.
    fn should_stop(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if self.node_limit.is_some_and(|limit| self.nodes >= limit) {
            self.stopped = true;
        } else if self.nodes.is_multiple_of(CHECK_INTERVAL) {
            self.stopped = self.stop.is_stopped()
                || self
                    .hard_deadline
                    .is_some_and(|deadline| Instant::now() >= deadline);
        }
        self.stopped
    }

    /// Plays `mv`, keeping the evaluation state in step.
    fn make(&mut self, mv: Move) {
        self.evaluator.on_make(&mut self.eval_state, &self.pos, mv);
        self.pos.make_move(mv);
    }

    fn make_null(&mut self) {
        self.evaluator
            .on_make(&mut self.eval_state, &self.pos, Move::NULL);
        self.pos.make_null_move();
    }

    /// Takes back the last move or null move.
    fn unmake(&mut self) {
        self.pos.unmake_move();
        self.evaluator.on_unmake(&mut self.eval_state);
    }

    fn evaluate(&self) -> i32 {
        self.evaluator
            .evaluate_incremental(&self.eval_state, &self.pos)
            .clamp(-MATE_BOUND + 1, MATE_BOUND - 1)
    }

    fn update_pv(&mut self, ply: usize, mv: Move) {
        let child_len = self
            .pv_len
            .get(ply + 1)
            .copied()
            .unwrap_or(ply + 1)
            .max(ply + 1);
        let (head, tail) = self.pv.split_at_mut(ply + 1);
        let row = &mut head[ply];
        row[ply] = mv;
        if let Some(child) = tail.first() {
            row[ply + 1..child_len].copy_from_slice(&child[ply + 1..child_len]);
        }
        self.pv_len[ply] = child_len;
    }

    fn negamax(
        &mut self,
        mut depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
        allow_null: bool,
    ) -> i32 {
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return 0;
        }
        let in_check = self.pos.in_check();
        // Check extension: never stop the search in the middle of a check.
        if in_check {
            depth += 1;
        }
        if depth <= 0 {
            return self.quiescence(alpha, beta, ply);
        }

        self.nodes += 1;
        self.seldepth = self.seldepth.max(ply);
        let pv_node = beta - alpha > 1;
        let root = ply == 0;

        if !root {
            if self.pos.halfmove_clock() >= 100
                || self.pos.is_repetition()
                || self.pos.is_insufficient_material()
            {
                return DRAW;
            }
            if ply >= MAX_PLY - 1 {
                return self.evaluate();
            }
            // Mate distance pruning: no line can beat a mate already found closer to the root.
            alpha = alpha.max(-MATE + ply as i32);
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        let key = self.pos.hash();
        let entry = self.tt.probe(key);
        let tt_move = entry.map_or(Move::NULL, |e| e.best_move);
        if let Some(entry) = entry
            && !pv_node
            && i32::from(entry.depth) >= depth
        {
            let score = from_tt(i32::from(entry.score), ply);
            let cutoff = match entry.bound {
                Bound::Exact => true,
                Bound::Lower => score >= beta,
                Bound::Upper => score <= alpha,
                Bound::None => false,
            };
            if cutoff {
                return score;
            }
        }

        let static_eval = if in_check { -INFINITY } else { self.evaluate() };

        if !pv_node && !in_check && beta.abs() < MATE_BOUND {
            // Reverse futility pruning: far above beta at low depth, assume it holds.
            if depth <= 6 && static_eval - 80 * depth >= beta {
                return static_eval;
            }
            // Null-move pruning: if passing still fails high, a real move will too.
            if allow_null
                && depth >= 3
                && static_eval >= beta
                && self.pos.has_non_pawn_material(self.pos.side_to_move())
            {
                let reduction = 3 + depth / 4 + ((static_eval - beta) / 200).min(3);
                self.make_null();
                let score = -self.negamax(depth - 1 - reduction, -beta, -beta + 1, ply + 1, false);
                self.unmake();
                if self.stopped {
                    return 0;
                }
                if score >= beta {
                    return if score >= MATE_BOUND { beta } else { score };
                }
            }
        }

        let moves = self.pos.legal_moves();
        if moves.is_empty() {
            return if in_check { -MATE + ply as i32 } else { DRAW };
        }

        let side = self.pos.side_to_move();
        let killers = self.killers.get(ply);
        let picker = MovePicker::new(&self.pos, moves, tt_move, killers, self.history);
        let mut quiets_tried = [Move::NULL; 64];
        let mut quiet_count = 0;
        let original_alpha = alpha;
        let mut best_score = -INFINITY;
        let mut best_move = Move::NULL;

        // Moves actually searched (excluded root moves do not count).
        let mut index = 0;
        for mv in picker {
            if root && self.excluded_root.contains(&mv) {
                continue;
            }
            let quiet = !is_noisy(mv);
            self.make(mv);
            let gives_check = self.pos.in_check();

            let score = if index == 0 {
                -self.negamax(depth - 1, -beta, -alpha, ply + 1, true)
            } else {
                // Late move reductions for quiet moves ordered late.
                let mut reduction = 0;
                if depth >= 3 && quiet && !in_check && !gives_check && index >= 2 {
                    reduction = LMR[depth.min(63) as usize][index.min(63)];
                    if pv_node {
                        reduction -= 1;
                    }
                    if mv == killers[0] || mv == killers[1] {
                        reduction -= 1;
                    }
                    reduction = reduction.clamp(0, depth - 2);
                }
                // Principal variation search: prove the move is worse with a
                // null window, re-search fully only if it is not.
                let mut score =
                    -self.negamax(depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, true);
                if score > alpha && reduction > 0 {
                    score = -self.negamax(depth - 1, -alpha - 1, -alpha, ply + 1, true);
                }
                if score > alpha && score < beta {
                    score = -self.negamax(depth - 1, -beta, -alpha, ply + 1, true);
                }
                score
            };
            self.unmake();
            if self.stopped {
                return 0;
            }

            if score > best_score {
                best_score = score;
                if score > alpha {
                    alpha = score;
                    best_move = mv;
                    self.update_pv(ply, mv);
                    if alpha >= beta {
                        if quiet {
                            self.reward_quiet(side, mv, depth, ply, &quiets_tried[..quiet_count]);
                        }
                        break;
                    }
                }
            }
            if quiet && quiet_count < quiets_tried.len() {
                quiets_tried[quiet_count] = mv;
                quiet_count += 1;
            }
            index += 1;
        }

        let bound = if best_score >= beta {
            Bound::Lower
        } else if best_score > original_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        self.tt
            .store(key, best_move, to_tt(best_score, ply), depth, bound);
        best_score
    }

    /// A quiet move caused a cutoff: remember it as a killer, raise its
    /// history, and lower the history of the quiet moves tried before it.
    fn reward_quiet(
        &mut self,
        side: chess_core::Color,
        mv: Move,
        depth: i32,
        ply: usize,
        tried: &[Move],
    ) {
        self.killers.add(ply, mv);
        let bonus = (depth * depth).min(1200);
        self.history.update(side, mv, bonus);
        for &other in tried {
            self.history.update(side, other, -bonus);
        }
    }

    /// Searches captures and promotions only, until the position is quiet,
    /// so that the static evaluation is never taken in the middle of an exchange.
    fn quiescence(&mut self, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.pv_len[ply] = ply;
        if self.should_stop() {
            return 0;
        }
        self.nodes += 1;
        self.seldepth = self.seldepth.max(ply);
        if ply >= MAX_PLY - 1 {
            return self.evaluate();
        }

        let in_check = self.pos.in_check();
        let mut best_score = -INFINITY;
        if !in_check {
            // Stand pat: the side to move may decline every capture.
            best_score = self.evaluate();
            if best_score >= beta {
                return best_score;
            }
            alpha = alpha.max(best_score);
        }

        // In check every evasion must be considered, otherwise mates are missed.
        let moves = if in_check {
            self.pos.legal_moves()
        } else {
            self.pos.legal_noisy_moves()
        };
        if in_check && moves.is_empty() {
            return -MATE + ply as i32;
        }
        let picker = MovePicker::new(&self.pos, moves, Move::NULL, [Move::NULL; 2], self.history);
        for mv in picker {
            if !in_check
                && matches!(mv.promotion(), Some(kind) if kind != chess_core::PieceKind::Queen)
            {
                continue;
            }
            self.make(mv);
            let score = -self.quiescence(-beta, -alpha, ply + 1);
            self.unmake();
            if self.stopped {
                return 0;
            }
            if score > best_score {
                best_score = score;
                if score > alpha {
                    alpha = score;
                    self.update_pv(ply, mv);
                    if alpha >= beta {
                        break;
                    }
                }
            }
        }
        best_score
    }
}
