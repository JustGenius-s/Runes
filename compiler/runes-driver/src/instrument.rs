//! MIR instrumentation: split call edges and insert runtime hooks.
//!
//! Each traced call is rewritten as
//! `enter(token) -> <original call> -> exit(token) -> original target`, and each
//! derived assignment gets a `value.derive` hook after it.

use std::collections::HashMap;

use rustc_middle::mir::{
    BasicBlockData, Body, CallSource, LocalDecl, Operand, Terminator, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::DefId;
use rustc_span::{Pos, Span, Spanned};

use crate::hooks::{
    dependency_item, function_operand, roots_operand, site_value_id, str_operand, u32_operand,
    u64_operand,
};
use crate::provenance::{Roots, traced_assignments};

/// Source file, line and column (1-based) for a span.
fn call_site(tcx: TyCtxt<'_>, span: Span) -> (String, u32, u32) {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    (
        location
            .file
            .name
            .prefer_local_unconditionally()
            .to_string(),
        u32::try_from(location.line).unwrap_or(u32::MAX),
        u32::try_from(location.col.to_usize() + 1).unwrap_or(u32::MAX),
    )
}

/// Emits a `trace_value_derive` hook after every assignment that derives a
/// value from traced data.
pub fn instrument_assignments<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    local_roots: &HashMap<rustc_middle::mir::Local, Roots>,
) {
    let Some(derive_hook) = dependency_item(tcx, "runes_runtime", "trace_value_derive") else {
        return;
    };
    let assignments = traced_assignments(body, local_roots);
    for assignment in assignments {
        let block = rustc_middle::mir::BasicBlock::from_usize(assignment.block_index);
        let source_info =
            body.basic_blocks[block].statements[assignment.statement_index].source_info;
        if body.basic_blocks[block].is_cleanup {
            continue;
        }

        let remaining = body.basic_blocks_mut()[block]
            .statements
            .split_off(assignment.statement_index + 1);
        let terminator = body.basic_blocks_mut()[block].terminator.take();
        let continuation = body
            .basic_blocks_mut()
            .push(BasicBlockData::new_stmts(remaining, terminator, false));
        let unit = body
            .local_decls
            .push(LocalDecl::new(tcx.types.unit, assignment.span));
        let (file, line, column) = call_site(tcx, assignment.span);

        body.basic_blocks_mut()[block].terminator = Some(Terminator {
            source_info,
            kind: TerminatorKind::Call {
                func: function_operand(tcx, derive_hook, assignment.span),
                args: vec![
                    Spanned {
                        node: str_operand(tcx, &assignment.label, assignment.span),
                        span: assignment.span,
                    },
                    Spanned {
                        node: str_operand(tcx, &file, assignment.span),
                        span: assignment.span,
                    },
                    Spanned {
                        node: u32_operand(tcx, line, assignment.span),
                        span: assignment.span,
                    },
                    Spanned {
                        node: u32_operand(tcx, column, assignment.span),
                        span: assignment.span,
                    },
                    Spanned {
                        node: roots_operand(tcx, &assignment.roots, assignment.span),
                        span: assignment.span,
                    },
                    Spanned {
                        node: u64_operand(
                            tcx,
                            site_value_id(&file, line, column),
                            assignment.span,
                        ),
                        span: assignment.span,
                    },
                ]
                .into_boxed_slice(),
                destination: unit.into(),
                target: Some(continuation),
                unwind: UnwindAction::Continue,
                call_source: CallSource::Normal,
                fn_span: assignment.span,
            },
            attributes: Default::default(),
        });
    }
}

/// Wraps every statically resolvable call in `enter`/`exit` hooks so the trace
/// records nesting, duration and provenance.
pub fn instrument_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    call_roots: &HashMap<usize, Roots>,
) {
    let (Some(enter_hook), Some(exit_hook)) = (
        dependency_item(tcx, "runes_runtime", "trace_call_enter"),
        dependency_item(tcx, "runes_runtime", "trace_call_exit"),
    ) else {
        return;
    };
    if std::env::var_os("RUNES_DEBUG_INSTRUMENT").is_some() {
        eprintln!(
            "RUNES_INSTRUMENT {} call_roots={call_roots:?}",
            tcx.def_path_str(body.source.def_id())
        );
    }
    let original_block_count = body.basic_blocks.len();

    for block_index in 0..original_block_count {
        let block = rustc_middle::mir::BasicBlock::from_usize(block_index);
        if body.basic_blocks[block].is_cleanup {
            continue;
        }
        let Some(original) = body.basic_blocks[block].terminator.clone() else {
            continue;
        };
        let TerminatorKind::Call {
            func,
            target: Some(original_target),
            ..
        } = &original.kind
        else {
            continue;
        };
        let Some((callee, _)) = func.const_fn_def() else {
            continue;
        };
        let callee_name = tcx.def_path_str(callee);
        if callee_name.contains("runes_runtime::") {
            continue;
        }

        let span = original.source_info.span;
        if span.from_expansion() {
            continue;
        }
        let source_info = original.source_info;
        let original_target = *original_target;
        let (file, line, column) = call_site(tcx, span);
        let token = body.local_decls.push(LocalDecl::new(tcx.types.u64, span));
        let unit = body.local_decls.push(LocalDecl::new(tcx.types.unit, span));

        let call_block = body
            .basic_blocks_mut()
            .push(BasicBlockData::new(None, false));
        let exit_block = body
            .basic_blocks_mut()
            .push(BasicBlockData::new(None, false));

        let mut traced_call = original;
        if let TerminatorKind::Call { target, .. } = &mut traced_call.kind {
            *target = Some(exit_block);
        }
        body.basic_blocks_mut()[call_block].terminator = Some(traced_call);

        body.basic_blocks_mut()[exit_block].terminator = Some(Terminator {
            source_info,
            kind: TerminatorKind::Call {
                func: function_operand(tcx, exit_hook, span),
                args: vec![Spanned {
                    node: Operand::Copy(token.into()),
                    span,
                }]
                .into_boxed_slice(),
                destination: unit.into(),
                target: Some(original_target),
                unwind: UnwindAction::Continue,
                call_source: CallSource::Normal,
                fn_span: span,
            },
            attributes: Default::default(),
        });

        let roots = call_roots.get(&block_index).cloned().unwrap_or_default();
        body.basic_blocks_mut()[block].terminator = Some(Terminator {
            source_info,
            kind: TerminatorKind::Call {
                func: function_operand(tcx, enter_hook, span),
                args: vec![
                    Spanned {
                        node: str_operand(tcx, &callee_name, span),
                        span,
                    },
                    Spanned {
                        node: str_operand(tcx, &file, span),
                        span,
                    },
                    Spanned {
                        node: u32_operand(tcx, line, span),
                        span,
                    },
                    Spanned {
                        node: u32_operand(tcx, column, span),
                        span,
                    },
                    Spanned {
                        node: roots_operand(tcx, &roots, span),
                        span,
                    },
                ]
                .into_boxed_slice(),
                destination: token.into(),
                target: Some(call_block),
                unwind: UnwindAction::Continue,
                call_source: CallSource::Normal,
                fn_span: span,
            },
            attributes: Default::default(),
        });
    }
}

/// Resolves the runtime hooks needed for instrumentation.
pub fn resolve_hooks(tcx: TyCtxt<'_>) -> Option<(DefId, DefId, DefId)> {
    Some((
        dependency_item(tcx, "runes_runtime", "trace_call_enter")?,
        dependency_item(tcx, "runes_runtime", "trace_call_exit")?,
        dependency_item(tcx, "runes_runtime", "trace_value_derive")?,
    ))
}
