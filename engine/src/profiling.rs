//! Low-overhead engine counters for profiling search workloads.
//!
//! Counters are active only with the `profiling` Cargo feature. Production
//! builds keep the call sites but compile their bodies to no-ops.

#[cfg(feature = "profiling")]
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProfilingSnapshot {
    pub legal_move_generation_calls: u64,
    pub drop_generation_calls: u64,
    pub attack_map_generation_calls: u64,
    pub chessembly_run_calls: u64,
    pub placement_generation_calls: u64,
    pub legal_move_generation_nanos: u64,
    pub placement_generation_nanos: u64,
    pub generated_move_candidates: u64,
    pub generated_drop_candidates: u64,
    pub chessembly_cache_hits: u64,
    pub chessembly_cache_rebuilds: u64,
}

#[cfg(feature = "profiling")]
mod enabled {
    use super::ProfilingSnapshot;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static LEGAL_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static DROP_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static ATTACK_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static CHESSEMBLY_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static PLACEMENT_CALLS: AtomicU64 = AtomicU64::new(0);
    pub static LEGAL_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static PLACEMENT_NANOS: AtomicU64 = AtomicU64::new(0);
    pub static MOVE_CANDIDATES: AtomicU64 = AtomicU64::new(0);
    pub static DROP_CANDIDATES: AtomicU64 = AtomicU64::new(0);
    pub static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
    pub static CACHE_REBUILDS: AtomicU64 = AtomicU64::new(0);

    pub fn add(counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    pub fn snapshot() -> ProfilingSnapshot {
        let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        ProfilingSnapshot {
            legal_move_generation_calls: load(&LEGAL_CALLS),
            drop_generation_calls: load(&DROP_CALLS),
            attack_map_generation_calls: load(&ATTACK_CALLS),
            chessembly_run_calls: load(&CHESSEMBLY_CALLS),
            placement_generation_calls: load(&PLACEMENT_CALLS),
            legal_move_generation_nanos: load(&LEGAL_NANOS),
            placement_generation_nanos: load(&PLACEMENT_NANOS),
            generated_move_candidates: load(&MOVE_CANDIDATES),
            generated_drop_candidates: load(&DROP_CANDIDATES),
            chessembly_cache_hits: load(&CACHE_HITS),
            chessembly_cache_rebuilds: load(&CACHE_REBUILDS),
        }
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
recorder!(record_cache_rebuild, CACHE_REBUILDS);

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
