//! Runtime hooks targeted by the native Runes compiler.
//!
//! `mark_root` deliberately preserves the value's Rust type. A future MIR pass
//! recognizes this call and attaches shadow provenance without wrapping `T`.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub use runes_trace::{SourceSite, TraceEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootMarkerEvent {
    pub site: SourceSite,
    pub at_ns: u128,
}

struct MarkerState {
    started: Instant,
    roots: Vec<RootMarkerEvent>,
    last_root: Option<SourceSite>,
    timeline: Vec<TraceEvent>,
}

static MARKERS: OnceLock<Mutex<MarkerState>> = OnceLock::new();
static NEXT_CALL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct CallFrame {
    id: u64,
    label: &'static str,
    started: Instant,
    traced: bool,
    root: Option<&'static str>,
}

thread_local! {
    static CALL_STACK: RefCell<Vec<CallFrame>> = const { RefCell::new(Vec::new()) };
}

fn markers() -> &'static Mutex<MarkerState> {
    MARKERS.get_or_init(|| {
        Mutex::new(MarkerState {
            started: Instant::now(),
            roots: Vec::new(),
            last_root: None,
            timeline: Vec::new(),
        })
    })
}

fn record_event(kind: &'static str, detail: String) -> u128 {
    let mut state = markers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let at_ns = state.started.elapsed().as_nanos();
    state.timeline.push(TraceEvent {
        at_ns,
        kind,
        detail,
    });
    at_ns
}

fn latest_root() -> Option<&'static str> {
    let state = markers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.last_root.map(|site| site.binding)
}

/// Marks a native Rust value as a Runes trace root while preserving `T`.
///
/// The side effect makes the marker observable today. The MIR backend will use
/// the same function as its root intrinsic and propagate metadata afterwards.
#[inline(never)]
pub fn mark_root<T>(value: T, site: SourceSite) -> T {
    let mut state = markers()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let at_ns = state.started.elapsed().as_nanos();
    state.roots.push(RootMarkerEvent { site, at_ns });
    state.last_root = Some(site);
    state.timeline.push(TraceEvent {
        at_ns,
        kind: "root",
        detail: format!(
            "{} @ {}:{}:{}",
            site.binding, site.file, site.line, site.column
        ),
    });
    println!(
        "[Runes +{at_ns}ns] root       {} @ {}:{}:{}",
        site.binding, site.file, site.line, site.column
    );
    value
}

/// Compiler-inserted hook immediately before a native Rust call.
///
/// `seed` is true when MIR shadow analysis found that at least one argument
/// descends from a `rune` root. Calls nested inside a traced call inherit the
/// active trace context without changing their Rust values or signatures.
#[doc(hidden)]
#[inline(never)]
pub fn trace_call_enter(
    label: &'static str,
    file: &'static str,
    line: u32,
    column: u32,
    seed: bool,
) -> u64 {
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let traced = seed || stack.last().is_some_and(|frame| frame.traced);
        let root = if seed {
            latest_root()
        } else {
            stack.last().and_then(|frame| frame.root)
        };
        let id = if traced {
            NEXT_CALL_ID.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };
        if traced {
            let roots = root.map_or_else(|| "[]".to_owned(), |name| format!("[{name}]"));
            let at_ns = record_event(
                "call.enter",
                format!("#{id} {label} roots={roots} @ {file}:{line}:{column}"),
            );
            println!(
                "[Runes +{at_ns}ns] call.enter #{id} {label} roots={roots} @ {file}:{line}:{column}"
            );
        }
        stack.push(CallFrame {
            id,
            label,
            started: Instant::now(),
            traced,
            root,
        });
        id
    })
}

/// Compiler-inserted hook on the normal return edge of a native Rust call.
#[doc(hidden)]
#[inline(never)]
pub fn trace_call_exit(id: u64) {
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let Some(frame) = stack.pop() else {
            return;
        };
        debug_assert_eq!(frame.id, id);
        if frame.traced {
            let duration_ns = frame.started.elapsed().as_nanos();
            let roots = frame
                .root
                .map_or_else(|| "[]".to_owned(), |name| format!("[{name}]"));
            let at_ns = record_event(
                "call.exit",
                format!(
                    "#{id} {} roots={roots} duration={duration_ns}ns",
                    frame.label
                ),
            );
            println!(
                "[Runes +{at_ns}ns] call.exit  #{id} {} roots={roots} duration={duration_ns}ns",
                frame.label
            );
        }
    });
}

/// Compiler-inserted hook after a MIR assignment derived from traced data.
#[doc(hidden)]
#[inline(never)]
pub fn trace_value_derive(
    label: &'static str,
    file: &'static str,
    line: u32,
    column: u32,
    seed: bool,
) {
    CALL_STACK.with(|stack| {
        let stack = stack.borrow();
        let inherited_root = stack
            .last()
            .filter(|frame| frame.traced)
            .and_then(|frame| frame.root);
        let traced = seed || inherited_root.is_some();
        if !traced {
            return;
        }
        let root = if seed { latest_root() } else { inherited_root };
        let roots = root.map_or_else(|| "[]".to_owned(), |name| format!("[{name}]"));
        let at_ns = record_event(
            "value.derive",
            format!("{label} roots={roots} @ {file}:{line}:{column}"),
        );
        println!("[Runes +{at_ns}ns] value.derive {label} roots={roots} @ {file}:{line}:{column}");
    });
}

pub fn take_root_markers() -> Vec<RootMarkerEvent> {
    let Some(markers) = MARKERS.get() else {
        return Vec::new();
    };
    let mut state = markers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut state.roots)
}

/// Removes and returns the complete structured runtime timeline.
pub fn take_trace_events() -> Vec<TraceEvent> {
    let Some(markers) = MARKERS.get() else {
        return Vec::new();
    };
    let mut state = markers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut state.timeline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_marker_preserves_the_value_and_records_its_site() {
        take_root_markers();
        take_trace_events();
        let value = mark_root(
            String::from("Ada"),
            SourceSite::new("user", "demo.rs", 4, 9),
        );

        assert_eq!(value, "Ada");
        let call = trace_call_enter("normalize_name", "demo.rs", 6, 5, true);
        trace_value_derive("user.name", "demo.rs", 7, 9, false);
        trace_call_exit(call);

        let roots = take_root_markers();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].site.binding, "user");
        let timeline = take_trace_events();
        assert_eq!(timeline.len(), 4);
        assert_eq!(timeline[0].kind, "root");
        assert_eq!(timeline[1].kind, "call.enter");
        assert_eq!(timeline[2].kind, "value.derive");
        assert_eq!(timeline[3].kind, "call.exit");
    }
}
