//! Runtime hooks targeted by the native Runes compiler.
//!
//! `mark_root` and `mark_root_at` deliberately preserve the value's Rust type.
//! The MIR pass recognizes these calls and attaches shadow provenance without
//! wrapping `T`.
//!
//! Events are recorded into a per-thread fixed-capacity ring buffer with no
//! locking on the hot path. No file IO happens while tracing: consumers call
//! [`write_trace_file`] once at the end of `main` to persist a structured JSON
//! document suitable for a frontend, or [`take_trace_events`] to collect the
//! events in memory.

use std::cell::RefCell;
use std::io;
use std::mem::MaybeUninit;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub use runes_trace::{CallId, SourceSite, TraceEvent, ValueId};

const DEFAULT_BUFFER_CAPACITY: usize = 4096;

/// Registers a panic hook exactly once. The hook runs on the panicking thread,
/// so it can access that thread's `CALL_STACK`, drain every frame that unwind
/// skipped, and emit a paired `call.exit` event for each.
fn ensure_panic_hook() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        drain_call_stack_on_unwind();
        previous(info);
    }));
}

/// Pops every `CallFrame` the current unwind skipped and records a paired
/// `call.exit` event for each traced frame. Also called implicitly by the panic
/// hook, so `catch_unwind` followed by normal code does not inherit stale roots.
fn drain_call_stack_on_unwind() {
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        while let Some(frame) = stack.pop() {
            if !frame.traced {
                continue;
            }
            let duration_ns = frame.started.elapsed().as_nanos();
            let event = TraceEvent::CallExit {
                at_ns: started().elapsed().as_nanos(),
                call_id: frame.id,
                label: frame.label.to_owned(),
                roots: frame.roots.clone(),
                duration_ns,
                unwind: true,
            };
            let at_ns = event.at_ns();
            record_event(event.clone());
            if print_enabled() {
                println!("[Runes +{at_ns}ns] {}", render_human(&event));
            }
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootMarkerEvent {
    pub site: SourceSite,
    pub at_ns: u128,
}

/// Unified monotonic clock base shared by every thread, so `at_ns` values are
/// comparable across the whole process.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Events drained from threads that have already exited.
static FLUSHED_EVENTS: OnceLock<Mutex<Vec<TraceEvent>>> = OnceLock::new();
static FLUSHED_ROOTS: OnceLock<Mutex<Vec<RootMarkerEvent>>> = OnceLock::new();
/// Number of events dropped because a thread buffer was full.
static OVERFLOWED: AtomicU64 = AtomicU64::new(0);

static NEXT_CALL_ID: AtomicU64 = AtomicU64::new(1);

fn started() -> Instant {
    *STARTED.get_or_init(Instant::now)
}

fn buffer_capacity() -> usize {
    std::env::var("RUNES_BUFFER_CAPACITY")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_BUFFER_CAPACITY)
        .max(1)
}

fn print_enabled() -> bool {
    std::env::var("RUNES_PRINT").map_or(true, |value| value != "0")
}

/// A fixed-capacity ring buffer of `TraceEvent`s. Drops new events once full
/// and counts them in `overflowed`.
struct EventRing {
    capacity: usize,
    slots: Vec<MaybeUninit<TraceEvent>>,
    /// Index of the next write slot.
    head: usize,
    /// Number of live events.
    count: usize,
    overflowed: u64,
}

impl EventRing {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            slots: (0..capacity).map(|_| MaybeUninit::uninit()).collect(),
            head: 0,
            count: 0,
            overflowed: 0,
        }
    }

    fn push(&mut self, event: TraceEvent) {
        if self.count == self.capacity {
            self.overflowed += 1;
            return;
        }
        self.slots[self.head].write(event);
        self.head = (self.head + 1) % self.capacity;
        self.count += 1;
    }

    /// Removes all events in insertion order and returns them, resetting the
    /// overflow counter alongside.
    fn drain(&mut self) -> (Vec<TraceEvent>, u64) {
        let oldest = (self.head + self.capacity - self.count) % self.capacity;
        let mut out = Vec::with_capacity(self.count);
        for offset in 0..self.count {
            let index = (oldest + offset) % self.capacity;
            // SAFETY: every slot in [oldest, oldest + count) was written by
            // `push` and has not been read yet; each is read exactly once.
            out.push(unsafe { self.slots[index].assume_init_read() });
        }
        self.head = 0;
        self.count = 0;
        let overflowed = self.overflowed;
        self.overflowed = 0;
        (out, overflowed)
    }
}

impl Drop for EventRing {
    fn drop(&mut self) {
        let oldest = (self.head + self.capacity - self.count) % self.capacity;
        for offset in 0..self.count {
            let index = (oldest + offset) % self.capacity;
            // SAFETY: same reasoning as `drain`, but dropping in place.
            unsafe { self.slots[index].assume_init_drop() };
        }
    }
}

/// Per-thread state. Root markers are kept separately because their structured
/// `site` does not fit into a `TraceEvent`.
struct ThreadBuffer {
    roots: Vec<RootMarkerEvent>,
    events: EventRing,
}

impl ThreadBuffer {
    fn new() -> Self {
        Self {
            roots: Vec::new(),
            events: EventRing::new(buffer_capacity()),
        }
    }
}

impl Drop for ThreadBuffer {
    fn drop(&mut self) {
        let roots = std::mem::take(&mut self.roots);
        let (events, overflowed) = self.events.drain();
        if overflowed > 0 {
            OVERFLOWED.fetch_add(overflowed, Ordering::Relaxed);
        }
        if !roots.is_empty() {
            FLUSHED_ROOTS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend(roots);
        }
        if !events.is_empty() {
            FLUSHED_EVENTS
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend(events);
        }
    }
}

thread_local! {
    static BUFFER: RefCell<ThreadBuffer> = RefCell::new(ThreadBuffer::new());
}

#[derive(Debug)]
struct CallFrame {
    id: u64,
    label: &'static str,
    started: Instant,
    traced: bool,
    roots: Vec<String>,
}

thread_local! {
    static CALL_STACK: RefCell<Vec<CallFrame>> = const { RefCell::new(Vec::new()) };
}

fn record_event(event: TraceEvent) -> u128 {
    let at_ns = event.at_ns();
    BUFFER.with(|buffer| buffer.borrow_mut().events.push(event));
    at_ns
}

/// Parses a comma-separated provenance string into its root names. An empty
/// string denotes "no direct seed": the caller inherits the enclosing frame's
/// roots instead.
fn parse_roots(roots: &str) -> Vec<String> {
    roots
        .split(',')
        .filter(|root| !root.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Formats the trace roots carried by a frame or event. An empty set renders as
/// `[]`; a single root renders as `[name]`; several render as `[name, other]`.
fn roots_banner(roots: &[String]) -> String {
    let mut banner = String::from("[");
    for (index, root) in roots.iter().enumerate() {
        if index > 0 {
            banner.push_str(", ");
        }
        banner.push_str(root);
    }
    banner.push(']');
    banner
}

/// Human-readable single-line rendering for `println!`. The structured event
/// itself is what consumers persist; this is only for terminal output.
fn render_human(event: &TraceEvent) -> String {
    match event {
        TraceEvent::Root {
            binding,
            file,
            line,
            column,
            ..
        } => format!("root       {binding} @ {file}:{line}:{column}"),
        TraceEvent::CallEnter {
            call_id,
            label,
            file,
            line,
            column,
            roots,
            ..
        } => {
            let roots = roots_banner(roots);
            format!("call.enter #{call_id} {label} roots={roots} @ {file}:{line}:{column}")
        }
        TraceEvent::CallExit {
            call_id,
            label,
            roots,
            duration_ns,
            unwind,
            ..
        } => {
            let roots = roots_banner(roots);
            let unwind = if *unwind { " unwind=true" } else { "" };
            format!("call.exit  #{call_id} {label} roots={roots} duration={duration_ns}ns{unwind}")
        }
        TraceEvent::ValueDerive {
            label,
            file,
            line,
            column,
            roots,
            ..
        } => {
            let roots = roots_banner(roots);
            format!("value.derive {label} roots={roots} @ {file}:{line}:{column}")
        }
    }
}

/// Records a root marker without renaming the root: the driver supplies the
/// source site explicitly through `mark_root_at`.
#[inline(never)]
fn record_root(site: SourceSite) {
    let at_ns = started().elapsed().as_nanos();
    let event = TraceEvent::Root {
        at_ns,
        binding: site.binding.to_owned(),
        file: site.file.to_owned(),
        line: site.line,
        column: site.column,
    };
    BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        buffer.roots.push(RootMarkerEvent { site, at_ns });
        buffer.events.push(event);
    });
    if print_enabled() {
        println!(
            "[Runes +{at_ns}ns] root       {} @ {}:{}:{}",
            site.binding, site.file, site.line, site.column
        );
    }
}

/// Marks a native Rust value as a Runes trace root while preserving `T`.
///
/// The binding name is attached afterwards by `mark_root_at`; direct callers
/// still record a root with a placeholder binding.
#[inline(never)]
pub fn mark_root<T>(value: T, site: SourceSite) -> T {
    record_root(site);
    value
}

/// Marks a native Rust value as a Runes trace root, preserving `T`, with the
/// binding name attached as a separate argument. This is the lowering target
/// for `rune name = expression;`.
#[inline(never)]
pub fn mark_root_at<T>(value: T, binding: &'static str, site: SourceSite) -> T {
    let site = SourceSite::new(binding, site.file, site.line, site.column);
    record_root(site);
    value
}

/// Compiler-inserted hook immediately before a native Rust call.
///
/// `roots` is the provenance set established by MIR shadow analysis: a
/// comma-separated list of root binding names whose data flows into at least
/// one argument. An empty string means "no direct seed", in which case the
/// call inherits the enclosing frame's roots. Calls nested inside a traced
/// call inherit the active trace context without changing their Rust values or
/// signatures.
#[doc(hidden)]
#[inline(never)]
pub fn trace_call_enter(
    label: &'static str,
    file: &'static str,
    line: u32,
    column: u32,
    roots: &'static str,
) -> u64 {
    ensure_panic_hook();
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let mut traced_roots = parse_roots(roots);
        if traced_roots.is_empty()
            && let Some(parent) = stack.last()
        {
            traced_roots = parent.roots.clone();
        }
        let traced = !traced_roots.is_empty();
        let id = if traced {
            NEXT_CALL_ID.fetch_add(1, Ordering::Relaxed)
        } else {
            0
        };
        if traced {
            let event = TraceEvent::CallEnter {
                at_ns: started().elapsed().as_nanos(),
                call_id: id,
                label: label.to_owned(),
                file: file.to_owned(),
                line,
                column,
                roots: traced_roots.clone(),
            };
            let at_ns = record_event(event.clone());
            if print_enabled() {
                println!("[Runes +{at_ns}ns] {}", render_human(&event));
            }
        }
        stack.push(CallFrame {
            id,
            label,
            started: Instant::now(),
            traced,
            roots: traced_roots,
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
            let event = TraceEvent::CallExit {
                at_ns: started().elapsed().as_nanos(),
                call_id: id,
                label: frame.label.to_owned(),
                roots: frame.roots.clone(),
                duration_ns,
                unwind: false,
            };
            let at_ns = record_event(event.clone());
            if print_enabled() {
                println!("[Runes +{at_ns}ns] {}", render_human(&event));
            }
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
    roots: &'static str,
    value_id: u64,
) {
    CALL_STACK.with(|stack| {
        let stack = stack.borrow();
        let mut traced_roots = parse_roots(roots);
        if traced_roots.is_empty()
            && let Some(frame) = stack.last()
        {
            traced_roots = frame.roots.clone();
        }
        if traced_roots.is_empty() {
            return;
        }
        let event = TraceEvent::ValueDerive {
            at_ns: started().elapsed().as_nanos(),
            value_id,
            label: label.to_owned(),
            file: file.to_owned(),
            line,
            column,
            roots: traced_roots.clone(),
        };
        let at_ns = record_event(event.clone());
        if print_enabled() {
            println!("[Runes +{at_ns}ns] {}", render_human(&event));
        }
    });
}

/// Number of events dropped across the whole process because a thread buffer
/// filled up.
pub fn overflowed_event_count() -> u64 {
    OVERFLOWED.load(Ordering::Relaxed)
        + BUFFER.with(|buffer| buffer.borrow().events.overflowed)
}

/// Returns the root names currently active on this thread, in declaration
/// order. Used for manual propagation across boundaries the compiler cannot
/// see: indirect calls, dynamic dispatch, `thread::spawn`, and `async` tasks.
pub fn current_roots() -> Vec<String> {
    CALL_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .map(|frame| frame.roots.clone())
            .unwrap_or_default()
    })
}

/// Runs `f` with the given root names installed as the active context, so any
/// traced calls or derives performed inside inherit them. This is the manual
/// counterpart of the compiler's automatic context propagation: use it when
/// handing traced values to an indirect call, a new thread, or an async task.
pub fn with_roots<T>(roots: Vec<String>, f: impl FnOnce() -> T) -> T {
    CALL_STACK.with(|stack| {
        let traced = !roots.is_empty();
        stack.borrow_mut().push(CallFrame {
            id: 0,
            label: "with_roots",
            started: Instant::now(),
            traced,
            roots,
        });
    });
    let result = f();
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.last().is_some_and(|frame| frame.label == "with_roots") {
            stack.pop();
        }
    });
    result
}

pub fn take_root_markers() -> Vec<RootMarkerEvent> {
    let mut roots = BUFFER.with(|buffer| std::mem::take(&mut buffer.borrow_mut().roots));
    if let Some(flushed) = FLUSHED_ROOTS.get() {
        let mut flushed = flushed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        roots.extend(std::mem::take(&mut *flushed));
    }
    roots
}

/// Removes and returns the complete structured runtime timeline: the current
/// thread's buffered events followed by events drained from exited threads.
pub fn take_trace_events() -> Vec<TraceEvent> {
    let (mut events, overflowed) = BUFFER.with(|buffer| buffer.borrow_mut().events.drain());
    if overflowed > 0 {
        OVERFLOWED.fetch_add(overflowed, Ordering::Relaxed);
    }
    if let Some(flushed) = FLUSHED_EVENTS.get() {
        let mut flushed = flushed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        events.extend(std::mem::take(&mut *flushed));
    }
    events
}

/// Persists the accumulated timeline to `path` as a single JSON document,
/// sorted by timestamp, with no file IO performed on the tracing hot path.
///
/// The document has the shape
/// `{"schema_version":3,"overflowed_events":N,"events":[...]}` and is intended
/// to be consumed directly by a frontend for rendering a timeline, call
/// waterfall, or provenance graph.
pub fn write_trace_file(path: impl AsRef<Path>) -> io::Result<()> {
    let mut events = take_trace_events();
    events.sort_by_key(|event| event.at_ns());
    let overflowed = OVERFLOWED.load(Ordering::Relaxed);
    std::fs::write(path, render_json(&events, overflowed))
}

/// Writes the trace to the path in `RUNES_OUT`, or `trace.json` when unset.
/// Convenience wrapper so examples and end users share one policy without hard
/// coding a path.
pub fn write_trace_file_default() -> io::Result<()> {
    let path = std::env::var("RUNES_OUT").unwrap_or_else(|_| "trace.json".to_owned());
    write_trace_file(path)
}

fn render_json(events: &[TraceEvent], overflowed: u64) -> String {
    let mut out = String::from("{\"schema_version\":");
    out.push_str(&runes_trace::TRACE_SCHEMA_VERSION.to_string());
    out.push_str(",\"overflowed_events\":");
    out.push_str(&overflowed.to_string());
    out.push_str(",\"events\":[");
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&event_to_json(event));
    }
    out.push_str("]}\n");
    out
}

fn event_to_json(event: &TraceEvent) -> String {
    let mut out = String::from("{\"event\":\"");
    out.push_str(event.kind());
    out.push('"');
    match event {
        TraceEvent::Root {
            at_ns,
            binding,
            file,
            line,
            column,
        } => {
            push_field_u128(&mut out, "at_ns", *at_ns);
            push_field_str(&mut out, "binding", binding);
            push_field_str(&mut out, "file", file);
            push_field_u32(&mut out, "line", *line);
            push_field_u32(&mut out, "column", *column);
        }
        TraceEvent::CallEnter {
            at_ns,
            call_id,
            label,
            file,
            line,
            column,
            roots,
        } => {
            push_field_u128(&mut out, "at_ns", *at_ns);
            push_field_u64(&mut out, "call_id", *call_id);
            push_field_str(&mut out, "label", label);
            push_field_str(&mut out, "file", file);
            push_field_u32(&mut out, "line", *line);
            push_field_u32(&mut out, "column", *column);
            push_field_str_array(&mut out, "roots", roots);
        }
        TraceEvent::CallExit {
            at_ns,
            call_id,
            label,
            roots,
            duration_ns,
            unwind,
        } => {
            push_field_u128(&mut out, "at_ns", *at_ns);
            push_field_u64(&mut out, "call_id", *call_id);
            push_field_str(&mut out, "label", label);
            push_field_str_array(&mut out, "roots", roots);
            push_field_u128(&mut out, "duration_ns", *duration_ns);
            push_field_bool(&mut out, "unwind", *unwind);
        }
        TraceEvent::ValueDerive {
            at_ns,
            value_id,
            label,
            file,
            line,
            column,
            roots,
        } => {
            push_field_u128(&mut out, "at_ns", *at_ns);
            push_field_u64(&mut out, "value_id", *value_id);
            push_field_str(&mut out, "label", label);
            push_field_str(&mut out, "file", file);
            push_field_u32(&mut out, "line", *line);
            push_field_u32(&mut out, "column", *column);
            push_field_str_array(&mut out, "roots", roots);
        }
    }
    out.push('}');
    out
}

fn push_field_str(out: &mut String, key: &str, value: &str) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":\"");
    out.push_str(&json_escape(value));
    out.push('"');
}

fn push_field_str_array(out: &mut String, key: &str, values: &[String]) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&json_escape(value));
        out.push('"');
    }
    out.push(']');
}

fn push_field_u32(out: &mut String, key: &str, value: u32) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":");
    out.push_str(&value.to_string());
}

fn push_field_u64(out: &mut String, key: &str, value: u64) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":");
    out.push_str(&value.to_string());
}

fn push_field_u128(out: &mut String, key: &str, value: u128) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":");
    out.push_str(&value.to_string());
}

fn push_field_bool(out: &mut String, key: &str, value: bool) {
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":");
    out.push_str(if value { "true" } else { "false" });
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    static SERIAL: StdMutex<()> = StdMutex::new(());

    #[test]
    fn native_marker_preserves_the_value_and_records_its_site() {
        let _guard = SERIAL.lock().unwrap();
        take_root_markers();
        take_trace_events();
        let value = mark_root_at(
            String::from("Ada"),
            "user",
            SourceSite::new("", "demo.rs", 4, 9),
        );

        assert_eq!(value, "Ada");
        let call = trace_call_enter("normalize_name", "demo.rs", 6, 5, "user");
        trace_value_derive("user.name", "demo.rs", 7, 9, "", 42);
        trace_call_exit(call);

        let markers = take_root_markers();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].site.binding, "user");
        let timeline = take_trace_events();
        assert_eq!(timeline.len(), 4);
        assert!(matches!(timeline[0], TraceEvent::Root { .. }));
        assert!(matches!(timeline[1], TraceEvent::CallEnter { .. }));
        assert!(matches!(timeline[2], TraceEvent::ValueDerive { .. }));
        assert!(matches!(timeline[3], TraceEvent::CallExit { unwind: false, .. }));

        assert_eq!(timeline[0].roots(), vec!["user".to_owned()]);
        assert_eq!(timeline[1].roots(), vec!["user".to_owned()]);
        assert_eq!(timeline[2].roots(), vec!["user".to_owned()]);
        assert_eq!(timeline[3].roots(), vec!["user".to_owned()]);

        if let TraceEvent::ValueDerive { value_id, .. } = &timeline[2] {
            assert_eq!(*value_id, 42);
        } else {
            panic!("expected ValueDerive");
        }
    }

    #[test]
    fn inherits_parent_roots_when_the_seed_slice_is_empty() {
        let _guard = SERIAL.lock().unwrap();
        take_root_markers();
        take_trace_events();
        let outer = trace_call_enter("outer", "demo.rs", 1, 1, "a,b");
        trace_value_derive("field", "demo.rs", 2, 1, "", 7);
        trace_call_exit(outer);

        let timeline = take_trace_events();
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].roots(), vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(timeline[1].roots(), vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(timeline[2].roots(), vec!["a".to_owned(), "b".to_owned()]);
        if let TraceEvent::ValueDerive { value_id, .. } = &timeline[1] {
            assert_eq!(*value_id, 7);
        } else {
            panic!("expected ValueDerive");
        }
    }

    #[test]
    fn json_export_is_structured_and_frontend_friendly() {
        let _guard = SERIAL.lock().unwrap();
        take_root_markers();
        take_trace_events();
        mark_root_at(
            String::from("Ada"),
            "user",
            SourceSite::new("", "demo.rs", 4, 9),
        );
        let json = render_json(&take_trace_events(), 0);
        assert!(json.contains("\"schema_version\":3"));
        assert!(json.contains("\"event\":\"root\""));
        assert!(json.contains("\"binding\":\"user\""));
        assert!(json.contains("\"file\":\"demo.rs\""));
    }

    #[test]
    fn ring_buffer_drops_newest_when_full_and_counts_overflow() {
        let mut ring = EventRing::new(2);
        ring.push(TraceEvent::Root {
            at_ns: 1,
            binding: "a".to_owned(),
            file: String::new(),
            line: 0,
            column: 0,
        });
        ring.push(TraceEvent::Root {
            at_ns: 2,
            binding: "b".to_owned(),
            file: String::new(),
            line: 0,
            column: 0,
        });
        ring.push(TraceEvent::Root {
            at_ns: 3,
            binding: "c".to_owned(),
            file: String::new(),
            line: 0,
            column: 0,
        });

        let (events, overflowed) = ring.drain();
        assert_eq!(overflowed, 1);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].at_ns(), 1);
        assert_eq!(events[1].at_ns(), 2);
    }

    #[test]
    fn exited_thread_buffer_is_flushed_and_collectable() {
        let _guard = SERIAL.lock().unwrap();
        take_trace_events();
        let handle = std::thread::spawn(|| {
            mark_root_at(
                String::from("x"),
                "worker",
                SourceSite::new("", "demo.rs", 1, 1),
            );
        });
        handle.join().unwrap();

        let timeline = take_trace_events();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].kind(), "root");
        assert_eq!(timeline[0].roots(), vec!["worker".to_owned()]);
    }

    #[test]
    fn panic_drains_call_stack_and_pairs_exit_events() {
        let _guard = SERIAL.lock().unwrap();
        take_root_markers();
        take_trace_events();

        let result = std::panic::catch_unwind(|| {
            trace_call_enter("boom", "demo.rs", 1, 1, "user");
            panic!("boom");
        });
        assert!(result.is_err());

        let timeline = take_trace_events();
        let enter = timeline
            .iter()
            .find(|event| matches!(event, TraceEvent::CallEnter { .. }));
        let exit = timeline
            .iter()
            .find(|event| matches!(event, TraceEvent::CallExit { unwind: true, .. }));
        assert!(enter.is_some());
        assert!(exit.is_some());
    }

    #[test]
    fn with_roots_installs_context_and_current_roots_reports_it() {
        let _guard = SERIAL.lock().unwrap();
        take_trace_events();
        with_roots(vec!["a".to_owned(), "b".to_owned()], || {
            assert_eq!(current_roots(), vec!["a".to_owned(), "b".to_owned()]);
        });
        assert!(current_roots().is_empty());
    }

    #[test]
    fn with_roots_carries_context_across_a_spawned_thread() {
        let _guard = SERIAL.lock().unwrap();
        take_trace_events();
        let roots = vec!["worker".to_owned()];
        let handle = std::thread::spawn({
            let roots = roots.clone();
            move || {
                with_roots(roots, || {
                    let id = trace_call_enter("process", "demo.rs", 1, 1, "");
                    trace_call_exit(id);
                })
            }
        });
        handle.join().unwrap();

        let timeline = take_trace_events();
        let enter = timeline
            .iter()
            .find(|event| matches!(event, TraceEvent::CallEnter { .. }));
        assert!(enter.is_some());
        assert_eq!(enter.unwrap().roots(), vec!["worker".to_owned()]);
    }

    #[test]
    fn write_trace_file_persists_sorted_json() {
        let _guard = SERIAL.lock().unwrap();
        take_root_markers();
        take_trace_events();
        mark_root_at(
            String::from("x"),
            "a",
            SourceSite::new("", "demo.rs", 1, 1),
        );
        let dir = std::env::temp_dir();
        let path = dir.join(format!("runes-test-{}.json", std::process::id()));
        write_trace_file(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(text.contains("\"schema_version\":3"));
        assert!(text.contains("\"event\":\"root\""));
    }
}
