//! Server-path profiling compiled only for the opt-in `profiling` feature.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ServerProfilingSnapshot {
    pub(crate) heartbeat_calls: u64,
    pub(crate) heartbeat_handler_nanos: u64,
    pub(crate) heartbeat_guard_nanos: u64,
    pub(crate) heartbeat_view_nanos: u64,
    pub(crate) heartbeat_serialization_nanos: u64,
    pub(crate) heartbeat_response_bytes: u64,
    pub(crate) piece_options_serialization_nanos: u64,
}

#[cfg(feature = "profiling")]
mod enabled {
    use super::ServerProfilingSnapshot;
    use std::sync::atomic::{AtomicU64, Ordering};

    static HEARTBEAT_CALLS: AtomicU64 = AtomicU64::new(0);
    static HEARTBEAT_HANDLER_NANOS: AtomicU64 = AtomicU64::new(0);
    static HEARTBEAT_GUARD_NANOS: AtomicU64 = AtomicU64::new(0);
    static HEARTBEAT_VIEW_NANOS: AtomicU64 = AtomicU64::new(0);
    static HEARTBEAT_SERIALIZATION_NANOS: AtomicU64 = AtomicU64::new(0);
    static HEARTBEAT_RESPONSE_BYTES: AtomicU64 = AtomicU64::new(0);
    static PIECE_OPTIONS_SERIALIZATION_NANOS: AtomicU64 = AtomicU64::new(0);

    fn add(counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    pub(crate) fn record_heartbeat(
        handler_nanos: u64,
        guard_nanos: u64,
        view_nanos: u64,
        serialization_nanos: u64,
        response_bytes: u64,
    ) {
        add(&HEARTBEAT_CALLS, 1);
        add(&HEARTBEAT_HANDLER_NANOS, handler_nanos);
        add(&HEARTBEAT_GUARD_NANOS, guard_nanos);
        add(&HEARTBEAT_VIEW_NANOS, view_nanos);
        add(&HEARTBEAT_SERIALIZATION_NANOS, serialization_nanos);
        add(&HEARTBEAT_RESPONSE_BYTES, response_bytes);
    }

    pub(crate) fn record_piece_options_serialization(nanos: u64) {
        add(&PIECE_OPTIONS_SERIALIZATION_NANOS, nanos);
    }

    pub(crate) fn snapshot() -> ServerProfilingSnapshot {
        let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        ServerProfilingSnapshot {
            heartbeat_calls: load(&HEARTBEAT_CALLS),
            heartbeat_handler_nanos: load(&HEARTBEAT_HANDLER_NANOS),
            heartbeat_guard_nanos: load(&HEARTBEAT_GUARD_NANOS),
            heartbeat_view_nanos: load(&HEARTBEAT_VIEW_NANOS),
            heartbeat_serialization_nanos: load(&HEARTBEAT_SERIALIZATION_NANOS),
            heartbeat_response_bytes: load(&HEARTBEAT_RESPONSE_BYTES),
            piece_options_serialization_nanos: load(&PIECE_OPTIONS_SERIALIZATION_NANOS),
        }
    }
}

#[cfg(feature = "profiling")]
#[allow(unused_imports)]
pub(crate) use enabled::{record_heartbeat, record_piece_options_serialization, snapshot};

#[cfg(not(feature = "profiling"))]
pub(crate) fn snapshot() -> ServerProfilingSnapshot {
    ServerProfilingSnapshot::default()
}
