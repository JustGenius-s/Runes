//! Stable, backend-independent trace records for Runes.

pub const TRACE_SCHEMA_VERSION: u32 = 3;

pub type ValueId = u64;
pub type TraceId = u64;
pub type CallId = u64;

/// A structured, backend-independent trace event.
///
/// Every event carries the fields a frontend needs to render a provenance
/// graph, call waterfall, or timeline without parsing free-form text:
/// source location, call identity (to pair enter/exit), label, duration, and
/// the full root set. No event embeds human-readable prose; consumers render
/// their own presentation from these fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
    Root {
        at_ns: u128,
        binding: String,
        file: String,
        line: u32,
        column: u32,
    },
    CallEnter {
        at_ns: u128,
        call_id: CallId,
        label: String,
        file: String,
        line: u32,
        column: u32,
        roots: Vec<String>,
    },
    CallExit {
        at_ns: u128,
        call_id: CallId,
        label: String,
        roots: Vec<String>,
        duration_ns: u128,
        unwind: bool,
    },
    ValueDerive {
        at_ns: u128,
        value_id: ValueId,
        label: String,
        file: String,
        line: u32,
        column: u32,
        roots: Vec<String>,
    },
}

impl TraceEvent {
    pub fn at_ns(&self) -> u128 {
        match self {
            TraceEvent::Root { at_ns, .. }
            | TraceEvent::CallEnter { at_ns, .. }
            | TraceEvent::CallExit { at_ns, .. }
            | TraceEvent::ValueDerive { at_ns, .. } => *at_ns,
        }
    }

    /// Discriminant used as the `event` field in the JSON export.
    pub fn kind(&self) -> &'static str {
        match self {
            TraceEvent::Root { .. } => "root",
            TraceEvent::CallEnter { .. } => "call_enter",
            TraceEvent::CallExit { .. } => "call_exit",
            TraceEvent::ValueDerive { .. } => "value_derive",
        }
    }

    pub fn roots(&self) -> &[String] {
        match self {
            TraceEvent::Root { binding, .. } => std::slice::from_ref(binding),
            TraceEvent::CallEnter { roots, .. }
            | TraceEvent::CallExit { roots, .. }
            | TraceEvent::ValueDerive { roots, .. } => roots,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    pub id: ValueId,
    pub roots: Vec<TraceId>,
    pub label: String,
    pub preview: String,
    pub parents: Vec<ValueId>,
    pub created_at_ns: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRoot {
    pub id: TraceId,
    pub name: String,
    pub value_id: ValueId,
}

/// Timing-free representation used to compare compiler backend runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTrace {
    pub roots: Vec<TraceRoot>,
    pub nodes: Vec<SemanticNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNode {
    pub id: ValueId,
    pub roots: Vec<TraceId>,
    pub label: String,
    pub preview: String,
    pub parents: Vec<ValueId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSite {
    pub binding: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl SourceSite {
    pub const fn new(binding: &'static str, file: &'static str, line: u32, column: u32) -> Self {
        Self {
            binding,
            file,
            line,
            column,
        }
    }
}
