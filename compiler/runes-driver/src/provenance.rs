//! Provenance analysis: which `rune` roots may flow into each MIR local.
//!
//! This is a conservative may-analysis. Each local carries the *set* of root
//! names that may reach it; sets only grow, so the fixed point always
//! converges. It is a union analysis, not a versioned one: once a local is
//! tainted by a root, later overwrites do not remove it.

use std::collections::{BTreeSet, HashMap};

use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{Local, Location, Operand, Rvalue};
use rustc_middle::ty::TyCtxt;

use crate::hooks::root_binding_name;

/// A provenance set: names of every `rune` root whose data may flow into a MIR
/// local or call. Sorted for deterministic event output.
pub type Roots = BTreeSet<String>;

/// Per-local provenance, and the seed set of every traced call (by block).
pub struct Provenance {
    pub local_roots: HashMap<Local, Roots>,
    pub call_roots: HashMap<usize, Roots>,
}

/// Collects the union of roots reaching every local referenced in an rvalue.
struct RootsCollectingVisitor<'a> {
    traced: &'a HashMap<Local, Roots>,
    roots: &'a mut Roots,
}

impl<'tcx> Visitor<'tcx> for RootsCollectingVisitor<'_> {
    fn visit_local(&mut self, local: Local, _context: PlaceContext, _location: Location) {
        if let Some(roots) = self.traced.get(&local) {
            self.roots.extend(roots.iter().cloned());
        }
    }
}

/// Union of the roots of every local read by `rvalue`.
fn rvalue_roots<'tcx>(rvalue: &Rvalue<'tcx>, traced: &HashMap<Local, Roots>) -> Roots {
    let mut roots = Roots::new();
    RootsCollectingVisitor {
        traced,
        roots: &mut roots,
    }
    .visit_rvalue(
        rvalue,
        Location {
            block: rustc_middle::mir::START_BLOCK,
            statement_index: 0,
        },
    );
    roots
}

/// Roots of the single place an operand reads, if any.
fn operand_roots(operand: &Operand<'_>, traced: &HashMap<Local, Roots>) -> Roots {
    let Some(place) = operand.place() else {
        return Roots::new();
    };
    traced.get(&place.local).cloned().unwrap_or_default()
}

/// Merges `source` into `target`; returns whether anything was added.
fn merge_roots(target: &mut Roots, source: &Roots) -> bool {
    let before = target.len();
    target.extend(source.iter().cloned());
    target.len() != before
}

/// Propagates provenance to a fixed point over the whole body.
///
/// `mark_root_at` seeds its destination local with the decoded binding name;
/// every assignment and call destination then merges the union of its sources.
pub fn propagate_roots<'tcx>(tcx: TyCtxt<'tcx>, body: &rustc_middle::mir::Body<'tcx>) -> Provenance {
    let mut local_roots: HashMap<Local, Roots> = HashMap::new();
    let mut call_roots: HashMap<usize, Roots> = HashMap::new();
    let mut changed = true;

    while changed {
        changed = false;
        for (block_index, block) in body.basic_blocks.iter().enumerate() {
            for statement in &block.statements {
                let rustc_middle::mir::StatementKind::Assign(assignment) = &statement.kind else {
                    continue;
                };
                let (destination, rvalue) = &**assignment;
                let roots = rvalue_roots(rvalue, &local_roots);
                if merge_roots(local_roots.entry(destination.local).or_default(), &roots) {
                    changed = true;
                }
            }

            let Some(terminator) = &block.terminator else {
                continue;
            };
            let rustc_middle::mir::TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } = &terminator.kind
            else {
                continue;
            };
            if let Some(binding) = root_binding_name(tcx, func, args) {
                let mut roots = Roots::new();
                roots.insert(binding);
                if merge_roots(local_roots.entry(destination.local).or_default(), &roots) {
                    changed = true;
                }
                continue;
            }
            let mut roots = Roots::new();
            for argument in args {
                roots.extend(operand_roots(&argument.node, &local_roots));
            }
            if !roots.is_empty() {
                call_roots
                    .entry(block_index)
                    .or_default()
                    .extend(roots.iter().cloned());
            }
            if merge_roots(local_roots.entry(destination.local).or_default(), &roots) {
                changed = true;
            }
        }
    }

    Provenance {
        local_roots,
        call_roots,
    }
}

/// A MIR assignment that derives a value from traced data.
#[derive(Debug)]
pub struct DerivedAssignment {
    pub block_index: usize,
    pub statement_index: usize,
    pub label: String,
    pub span: rustc_span::Span,
    pub roots: Roots,
}

/// Assignments whose rvalue reads traced data, in instrumentation order
/// (per block, last statement first so splitting does not shift indices).
pub fn traced_assignments(
    body: &rustc_middle::mir::Body<'_>,
    local_roots: &HashMap<Local, Roots>,
) -> Vec<DerivedAssignment> {
    let mut assignments = Vec::new();

    for (block_index, block) in body.basic_blocks.iter().enumerate() {
        for (statement_index, statement) in block.statements.iter().enumerate() {
            if statement.source_info.span.from_expansion() {
                continue;
            }
            let rustc_middle::mir::StatementKind::Assign(assignment) = &statement.kind else {
                continue;
            };
            let (destination, rvalue) = &**assignment;
            let roots = rvalue_roots(rvalue, local_roots);
            if roots.is_empty() {
                continue;
            }
            assignments.push(DerivedAssignment {
                block_index,
                statement_index,
                label: format!("{destination:?} <- {rvalue:?}"),
                span: statement.source_info.span,
                roots,
            });
        }
    }

    assignments.sort_by_key(|assignment| {
        (
            assignment.block_index,
            std::cmp::Reverse(assignment.statement_index),
        )
    });
    assignments
}
