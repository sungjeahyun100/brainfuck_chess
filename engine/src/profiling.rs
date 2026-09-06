//! Low-overhead engine counters for profiling search workloads.
//!
//! Counters are active only with the `profiling` Cargo feature. Production
//! builds keep the call sites but compile their bodies to no-ops.

#[cfg(feature = "profiling")]
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProfilingSnapshot {
    pub position_key_generation_calls: u64,
    pub legal_move_generation_calls: u64,
    pub piece_move_generation_calls: u64,
    pub piece_attack_generation_calls: u64,
    pub piece_options_calls: u64,
    pub drop_generation_calls: u64,
    pub attack_map_generation_calls: u64,
    pub chessembly_run_calls: u64,
    pub placement_generation_calls: u64,
    pub legal_move_generation_nanos: u64,
    pub piece_move_generation_nanos: u64,
    pub piece_attack_generation_nanos: u64,
    pub piece_options_nanos: u64,
    pub deduplication_nanos: u64,
    pub placement_generation_nanos: u64,
    pub generated_move_candidates: u64,
    pub generated_drop_candidates: u64,
    pub chessembly_cache_hits: u64,
    pub chessembly_cache_checks: u64,
    pub chessembly_cache_rebuilds: u64,
    pub evaluation_calls: u64,
    pub action_application_calls: u64,
}

impl ProfilingSnapshot {
    /// Returns the saturating counter delta since an earlier snapshot.
    pub fn since(self, earlier: Self) -> Self {
        Self {
            position_key_generation_calls: self
                .position_key_generation_calls
                .saturating_sub(earlier.position_key_generation_calls),
            legal_move_generation_calls: self
                .legal_move_generation_calls
                .saturating_sub(earlier.legal_move_generation_calls),
            piece_move_generation_calls: self
                .piece_move_generation_calls
                .saturating_sub(earlier.piece_move_generation_calls),
            piece_attack_generation_calls: self
                .piece_attack_generation_calls
                .saturating_sub(earlier.piece_attack_generation_calls),
            piece_options_calls: self
                .piece_options_calls
                .saturating_sub(earlier.piece_options_calls),
            drop_generation_calls: self
                .drop_generation_calls
                .saturating_sub(earlier.drop_generation_calls),
            attack_map_generation_calls: self
                .attack_map_generation_calls
                .saturating_sub(earlier.attack_map_generation_calls),
            chessembly_run_calls: self
                .chessembly_run_calls
                .saturating_sub(earlier.chessembly_run_calls),
            placement_generation_calls: self
                .placement_generation_calls
                .saturating_sub(earlier.placement_generation_calls),
            legal_move_generation_nanos: self
                .legal_move_generation_nanos
                .saturating_sub(earlier.legal_move_generation_nanos),
            piece_move_generation_nanos: self
                .piece_move_generation_nanos
                .saturating_sub(earlier.piece_move_generation_nanos),
            piece_attack_generation_nanos: self
                .piece_attack_generation_nanos
                .saturating_sub(earlier.piece_attack_generation_nanos),
            piece_options_nanos: self
                .piece_options_nanos
                .saturating_sub(earlier.piece_options_nanos),
            deduplication_nanos: self
                .deduplication_nanos
                .saturating_sub(earlier.deduplication_nanos),
            placement_generation_nanos: self
                .placement_generation_nanos
                .saturating_sub(earlier.placement_generation_nanos),
            generated_move_candidates: self
                .generated_move_candidates
                .saturating_sub(earlier.generated_move_candidates),
            generated_drop_candidates: self
                .generated_drop_candidates
                .saturating_sub(earlier.generated_drop_candidates),
            chessembly_cache_hits: self
                .chessembly_cache_hits
                .saturating_sub(earlier.chessembly_cache_hits),
            chessembly_cache_checks: self
                .chessembly_cache_checks
                .saturating_sub(earlier.chessembly_cache_checks),
            chessembly_cache_rebuilds: self
                .chessembly_cache_rebuilds
                .saturating_sub(earlier.chessembly_cache_rebuilds),
            evaluation_calls: self
                .evaluation_calls
                .saturating_sub(earlier.evaluation_calls),
            action_application_calls: self
                .action_application_calls
                .saturating_sub(earlier.action_application_calls),
        }
    }
}

#[cfg(feature = "profiling")]
mod enabled {
    use super::ProfilingSnapshot;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    thread_local! {
        static POSITION_KEY_GENERATION_CALLS: Cell<u64> = const { Cell::new(0) };
    }

    pub static LEGAL_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_MOVE_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_ATTACK_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_OPTIONS_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static DROP_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static ATTACK_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static CHESSEMBLY_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PLACEMENT_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static LEGAL_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_MOVE_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_ATTACK_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static PIECE_OPTIONS_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static DEDUP_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static PLACEMENT_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static MOVE_CANDIDATES: AtomicU64 = AtomicU64::new(0);
    pub static DROP_CANDIDATES: AtomicU64 = AtomicU64::new(0);
    pub static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    pub static CACHE_CHECKS: AtomicU64 = AtomicU64::new(0);
    pub static CACHE_REBUILDS: AtomicU64 = AtomicU64::new(0);
    pub static EVALUATION_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static ACTION_APPLICATION_CALLS: AtomicU64 = AtomicU64::new(0);

    pub fn add(counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    pub fn snapshot() -> ProfilingSnapshot {
        let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        ProfilingSnapshot {
            position_key_generation_calls: POSITION_KEY_GENERATION_CALLS.get(),
            legal_move_generation_calls: load(&LEGAL_CALLS),
            piece_move_generation_calls: load(&PIECE_MOVE_CALLS),
            piece_attack_generation_calls: load(&PIECE_ATTACK_CALLS),
            piece_options_calls: load(&PIECE_OPTIONS_CALLS),
            drop_generation_calls: load(&DROP_CALLS),
            attack_map_generation_calls: load(&ATTACK_CALLS),
            chessembly_run_calls: load(&CHESSEMBLY_CALLS),
            placement_generation_calls: load(&PLACEMENT_CALLS),
            legal_move_generation_nanos: load(&LEGAL_NANOS),
            piece_move_generation_nanos: load(&PIECE_MOVE_NANOS),
            piece_attack_generation_nanos: load(&PIECE_ATTACK_NANOS),
            piece_options_nanos: load(&PIECE_OPTIONS_NANOS),
            deduplication_nanos: load(&DEDUP_NANOS),
            placement_generation_nanos: load(&PLACEMENT_NANOS),
            generated_move_candidates: load(&MOVE_CANDIDATES),
            generated_drop_candidates: load(&DROP_CANDIDATES),
            chessembly_cache_hits: load(&CACHE_HITS),
            chessembly_cache_checks: load(&CACHE_CHECKS),
            chessembly_cache_rebuilds: load(&CACHE_REBUILDS),
            evaluation_calls: load(&EVALUATION_CALLS),
            action_application_calls: load(&ACTION_APPLICATION_CALLS),
        }
    }

    pub fn record_position_key_generation(value: u64) {
        POSITION_KEY_GENERATION_CALLS
            .set(POSITION_KEY_GENERATION_CALLS.get().saturating_add(value));
    }
}

macro_rules! recorder {
    ($name:ident, $counter:ident) => {
        pub(crate) fn $name(value: u64) {
            #[cfg(feature = "profiling")]
            enabled::add(&enabled::$counter, value);
            #[cfg(not(feature = "profiling"))]
            let _ = value;
        }
    };
}

recorder!(record_attack_map, ATTACK_CALLS);
recorder!(record_chessembly_run, CHESSEMBLY_CALLS);
recorder!(record_cache_hit, CACHE_HITS);
recorder!(record_cache_check, CACHE_CHECKS);
recorder!(record_cache_rebuild, CACHE_REBUILDS);
recorder!(record_evaluation, EVALUATION_CALLS);
recorder!(record_action_application, ACTION_APPLICATION_CALLS);
pub(crate) fn record_position_key_generation(value: u64) {
    #[cfg(feature = "profiling")]
    enabled::record_position_key_generation(value);
    #[cfg(not(feature = "profiling"))]
    let _ = value;
}

#[cfg(feature = "profiling")]
pub(crate) fn record_legal_moves(duration: Duration, candidates: usize) {
    #[cfg(feature = "profiling")]
    {
        enabled::add(&enabled::LEGAL_CALLS, 1);
        enabled::add(&enabled::LEGAL_NANOS, duration.as_nanos() as u64);
        enabled::add(&enabled::MOVE_CANDIDATES, candidates as u64);
    }
    #[cfg(not(feature = "profiling"))]
    let _ = (duration, candidates);
}

pub(crate) fn record_drops(candidates: usize) {
    #[cfg(feature = "profiling")]
    {
        enabled::add(&enabled::DROP_CALLS, 1);
        enabled::add(&enabled::DROP_CANDIDATES, candidates as u64);
    }
    #[cfg(not(feature = "profiling"))]
    let _ = candidates;
}

#[cfg(feature = "profiling")]
pub(crate) fn record_piece_moves(duration: std::time::Duration, candidates: usize) {
    enabled::add(&enabled::PIECE_MOVE_CALLS, 1);
    enabled::add(&enabled::PIECE_MOVE_NANOS, duration.as_nanos() as u64);
    enabled::add(&enabled::MOVE_CANDIDATES, candidates as u64);
}

#[cfg(feature = "profiling")]
pub(crate) fn record_piece_attacks(duration: std::time::Duration) {
    enabled::add(&enabled::PIECE_ATTACK_CALLS, 1);
    enabled::add(&enabled::PIECE_ATTACK_NANOS, duration.as_nanos() as u64);
}

#[cfg(feature = "profiling")]
pub(crate) fn record_deduplication(duration: std::time::Duration) {
    enabled::add(&enabled::DEDUP_NANOS, duration.as_nanos() as u64);
}

/// Records end-to-end `/pieces/:piece_id/options` handler work in profiling builds.
#[cfg(feature = "profiling")]
pub fn record_piece_options(duration: std::time::Duration) {
    enabled::add(&enabled::PIECE_OPTIONS_CALLS, 1);
    enabled::add(&enabled::PIECE_OPTIONS_NANOS, duration.as_nanos() as u64);
}

#[cfg(feature = "profiling")]
pub(crate) fn record_placement(duration: Duration) {
    #[cfg(feature = "profiling")]
    {
        enabled::add(&enabled::PLACEMENT_CALLS, 1);
        enabled::add(&enabled::PLACEMENT_NANOS, duration.as_nanos() as u64);
    }
    #[cfg(not(feature = "profiling"))]
    let _ = duration;
}

pub fn snapshot() -> ProfilingSnapshot {
    #[cfg(feature = "profiling")]
    return enabled::snapshot();
    #[cfg(not(feature = "profiling"))]
    ProfilingSnapshot::default()
}
