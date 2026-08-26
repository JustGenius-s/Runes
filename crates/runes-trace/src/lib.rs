//! Stable, backend-independent trace records for Runes.

pub const TRACE_SCHEMA_VERSION: u32 = 1;

pub type ValueId = u64;
pub type TraceId = u64;
pub type CallId = u64;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub at_ns: u128,
    pub kind: &'static str,
    pub detail: String,
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
