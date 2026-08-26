//! Runtime hooks targeted by the native Runes compiler.
//!
//! `mark_root` deliberately preserves the value's Rust type. A future MIR pass
//! recognizes this call and attaches shadow provenance without wrapping `T`.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub use runes_trace::SourceSite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootMarkerEvent {
    pub site: SourceSite,
    pub at_ns: u128,
}

struct MarkerState {
    started: Instant,
    events: Vec<RootMarkerEvent>,
}

static MARKERS: OnceLock<Mutex<MarkerState>> = OnceLock::new();

/// Marks a native Rust value as a Runes trace root while preserving `T`.
///
/// The side effect makes the marker observable today. The MIR backend will use
/// the same function as its root intrinsic and propagate metadata afterwards.
#[inline(never)]
pub fn mark_root<T>(value: T, site: SourceSite) -> T {
    let markers = MARKERS.get_or_init(|| {
        Mutex::new(MarkerState {
            started: Instant::now(),
            events: Vec::new(),
        })
    });
    let mut state = markers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let at_ns = state.started.elapsed().as_nanos();
    state.events.push(RootMarkerEvent { site, at_ns });
    value
}

pub fn take_root_markers() -> Vec<RootMarkerEvent> {
    let Some(markers) = MARKERS.get() else {
        return Vec::new();
    };
    let mut state = markers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut state.events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_marker_preserves_the_value_and_records_its_site() {
        take_root_markers();
        let value = mark_root(
            String::from("Ada"),
            SourceSite::new("user", "demo.rs", 4, 9),
        );

        assert_eq!(value, "Ada");
        let events = take_root_markers();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].site.binding, "user");
    }
}
