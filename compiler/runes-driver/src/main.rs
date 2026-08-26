#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashSet;
use std::env;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use runes_syntax::{SyntaxConfig, lower_rust_source};
use rustc_driver::{Callbacks, Compilation, run_compiler};
use rustc_hir::def::Res;
use rustc_interface::interface;
use rustc_middle::mir::interpret::CTFE_ALLOC_SALT;
use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{
    BasicBlockData, Body, CallSource, Const, ConstOperand, ConstValue, Local, LocalDecl, Location,
    Operand, Rvalue, Terminator, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::def_id::{DefId, LocalDefId};
use rustc_span::source_map::FileLoader;
use rustc_span::{Pos, Span, Spanned};

type OptimizedMirProvider = for<'tcx> fn(TyCtxt<'tcx>, LocalDefId) -> &'tcx Body<'tcx>;

static ORIGINAL_OPTIMIZED_MIR: std::sync::OnceLock<OptimizedMirProvider> =
    std::sync::OnceLock::new();
static HAS_RUNE_SOURCE: AtomicBool = AtomicBool::new(false);

fn dependency_item(tcx: TyCtxt<'_>, crate_name: &str, item_name: &str) -> Option<DefId> {
    let crate_num = tcx
        .crates(())
        .iter()
        .copied()
        .find(|&crate_num| tcx.crate_name(crate_num).as_str() == crate_name)?;
    tcx.module_children(crate_num.as_def_id())
        .iter()
        .find_map(|child| {
            (child.ident.name.as_str() == item_name)
                .then(|| match child.res {
                    Res::Def(_, def_id) => Some(def_id),
                    _ => None,
                })
                .flatten()
        })
}

fn function_operand<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId, span: Span) -> Operand<'tcx> {
    let ty = tcx.type_of(def_id).instantiate_identity().skip_norm_wip();
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::zero_sized(ty),
    }))
}

fn str_operand<'tcx>(tcx: TyCtxt<'tcx>, value: &str, span: Span) -> Operand<'tcx> {
    let alloc_id = tcx.allocate_bytes_dedup(value.as_bytes(), CTFE_ALLOC_SALT);
    let ty = Ty::new_imm_ref(tcx, tcx.lifetimes.re_static, tcx.types.str_);
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::from_value(
            ConstValue::Slice {
                alloc_id,
                meta: value.len() as u64,
            },
            ty,
        ),
    }))
}

fn u32_operand<'tcx>(tcx: TyCtxt<'tcx>, value: u32, span: Span) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::from_bits(
            tcx,
            value.into(),
            ty::TypingEnv::fully_monomorphized(),
            tcx.types.u32,
        ),
    }))
}

fn bool_operand<'tcx>(tcx: TyCtxt<'tcx>, value: bool, span: Span) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::from_bool(tcx, value),
    }))
}

struct TracedLocalUse<'a> {
    traced: &'a HashSet<Local>,
    found: bool,
}

impl<'tcx> Visitor<'tcx> for TracedLocalUse<'_> {
    fn visit_local(&mut self, local: Local, _context: PlaceContext, _location: Location) {
        self.found |= self.traced.contains(&local);
    }
}

fn rvalue_uses_traced<'tcx>(rvalue: &Rvalue<'tcx>, traced: &HashSet<Local>) -> bool {
    let mut visitor = TracedLocalUse {
        traced,
        found: false,
    };
    visitor.visit_rvalue(
        rvalue,
        Location {
            block: rustc_middle::mir::START_BLOCK,
            statement_index: 0,
        },
    );
    visitor.found
}

fn operand_uses_traced(operand: &Operand<'_>, traced: &HashSet<Local>) -> bool {
    operand
        .place()
        .is_some_and(|place| traced.contains(&place.local))
}

fn is_root_marker(tcx: TyCtxt<'_>, func: &Operand<'_>) -> bool {
    func.const_fn_def().is_some_and(|(def_id, _)| {
        tcx.def_path_str(def_id)
            .ends_with("runes_runtime::mark_root")
    })
}

fn traced_call_seeds(tcx: TyCtxt<'_>, body: &Body<'_>) -> Vec<bool> {
    let mut traced = HashSet::new();
    let mut seeds = vec![false; body.basic_blocks.len()];
    let mut changed = true;

    while changed {
        changed = false;
        for (block_index, block) in body.basic_blocks.iter().enumerate() {
            for statement in &block.statements {
                if let rustc_middle::mir::StatementKind::Assign(assignment) = &statement.kind {
                    let (destination, rvalue) = &**assignment;
                    if rvalue_uses_traced(rvalue, &traced) {
                        changed |= traced.insert(destination.local);
                    }
                }
            }

            let Some(terminator) = &block.terminator else {
                continue;
            };
            let TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } = &terminator.kind
            else {
                continue;
            };
            if is_root_marker(tcx, func) {
                changed |= traced.insert(destination.local);
                continue;
            }
            let seed = args
                .iter()
                .any(|argument| operand_uses_traced(&argument.node, &traced));
            if seed {
                seeds[block_index] = true;
                changed |= traced.insert(destination.local);
            }
        }
    }
    seeds
}

#[derive(Debug)]
struct DerivedAssignment {
    block_index: usize,
    statement_index: usize,
    label: String,
    span: Span,
}

fn traced_assignments(tcx: TyCtxt<'_>, body: &Body<'_>) -> Vec<DerivedAssignment> {
    let mut traced = HashSet::new();
    let mut derived = HashSet::new();
    let mut changed = true;

    while changed {
        changed = false;
        for (block_index, block) in body.basic_blocks.iter().enumerate() {
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if let rustc_middle::mir::StatementKind::Assign(assignment) = &statement.kind {
                    let (destination, rvalue) = &**assignment;
                    if rvalue_uses_traced(rvalue, &traced) {
                        derived.insert((block_index, statement_index));
                        changed |= traced.insert(destination.local);
                    }
                }
            }
            let Some(terminator) = &block.terminator else {
                continue;
            };
            let TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } = &terminator.kind
            else {
                continue;
            };
            if is_root_marker(tcx, func)
                || args
                    .iter()
                    .any(|argument| operand_uses_traced(&argument.node, &traced))
            {
                changed |= traced.insert(destination.local);
            }
        }
    }

    let mut assignments = derived
        .into_iter()
        .filter_map(|(block_index, statement_index)| {
            let block = rustc_middle::mir::BasicBlock::from_usize(block_index);
            let statement = body.basic_blocks[block].statements.get(statement_index)?;
            if statement.source_info.span.from_expansion() {
                return None;
            }
            let rustc_middle::mir::StatementKind::Assign(assignment) = &statement.kind else {
                return None;
            };
            let (destination, rvalue) = &**assignment;
            Some(DerivedAssignment {
                block_index,
                statement_index,
                label: format!("{destination:?} <- {rvalue:?}"),
                span: statement.source_info.span,
            })
        })
        .collect::<Vec<_>>();
    assignments.sort_by_key(|assignment| {
        (
            assignment.block_index,
            std::cmp::Reverse(assignment.statement_index),
        )
    });
    assignments
}

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

fn instrument_assignments<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, derive_hook: DefId) {
    let assignments = traced_assignments(tcx, body);
    for assignment in assignments {
        let block = rustc_middle::mir::BasicBlock::from_usize(assignment.block_index);
        let source_info =
            body.basic_blocks[block].statements[assignment.statement_index].source_info;
        let is_cleanup = body.basic_blocks[block].is_cleanup;
        if is_cleanup {
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
                        node: bool_operand(tcx, true, assignment.span),
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

fn instrument_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    enter_hook: DefId,
    exit_hook: DefId,
) {
    let seeds = traced_call_seeds(tcx, body);
    if std::env::var_os("RUNES_DEBUG_INSTRUMENT").is_some() {
        eprintln!(
            "RUNES_INSTRUMENT {} seeds={seeds:?}",
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
                        node: bool_operand(tcx, seeds[block_index], span),
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

fn runes_optimized_mir<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> &'tcx Body<'tcx> {
    let original = ORIGINAL_OPTIMIZED_MIR
        .get()
        .expect("Runes optimized_mir provider was not installed");
    let mut body = original(tcx, def_id).clone();
    if HAS_RUNE_SOURCE.load(Ordering::Relaxed)
        && let Some(enter_hook) = dependency_item(tcx, "runes_runtime", "trace_call_enter")
        && let Some(exit_hook) = dependency_item(tcx, "runes_runtime", "trace_call_exit")
        && let Some(derive_hook) = dependency_item(tcx, "runes_runtime", "trace_value_derive")
    {
        instrument_assignments(tcx, &mut body, derive_hook);
        instrument_calls(tcx, &mut body, enter_hook, exit_hook);
    }
    tcx.arena.alloc(body)
}

#[derive(Default)]
struct RunesCallbacks {
    dump_mir: bool,
    scan_roots: bool,
    lower_keyword: bool,
    syntax: SyntaxConfig,
}

impl Callbacks for RunesCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        config.override_queries = Some(|_session, providers| {
            let original = providers.queries.optimized_mir;
            let _ = ORIGINAL_OPTIMIZED_MIR.set(original);
            providers.queries.optimized_mir = runes_optimized_mir;
        });
        if self.lower_keyword {
            config.file_loader = Some(Box::new(RunesFileLoader {
                syntax: self.syntax.clone(),
            }));
        }
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        if self.dump_mir || self.scan_roots {
            tcx.par_hir_body_owners(|def_id| {
                if !tcx.def_kind(def_id).is_fn_like() {
                    return;
                }
                let body = tcx.optimized_mir(def_id.to_def_id());
                if self.dump_mir {
                    eprintln!("RUNES_MIR {} {body:#?}", tcx.def_path_str(def_id));
                }
                if self.scan_roots {
                    for block in body.basic_blocks.iter() {
                        let Some(terminator) = &block.terminator else {
                            continue;
                        };
                        let TerminatorKind::Call { func, .. } = &terminator.kind else {
                            continue;
                        };
                        let Some((callee, _)) = func.const_fn_def() else {
                            continue;
                        };
                        if tcx
                            .def_path_str(callee)
                            .ends_with("runes_runtime::mark_root")
                        {
                            eprintln!(
                                "RUNES_ROOT {} at {:?}",
                                tcx.def_path_str(def_id),
                                terminator.source_info.span
                            );
                        }
                    }
                }
            });
        }
        Compilation::Continue
    }
}

struct RunesFileLoader {
    syntax: SyntaxConfig,
}

impl FileLoader for RunesFileLoader {
    fn current_directory(&self) -> io::Result<std::path::PathBuf> {
        env::current_dir()
    }

    fn file_exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        let source = std::fs::read_to_string(path)?;
        let lowered = lower_rust_source(&source, &self.syntax)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        if lowered != source {
            HAS_RUNE_SOURCE.store(true, Ordering::Relaxed);
        }
        Ok(lowered)
    }

    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
        std::fs::read(path).map(Arc::from)
    }
}

fn main() -> ExitCode {
    let mut rustc_args = Vec::new();
    let mut dump_mir = env::var_os("RUNES_DUMP_MIR").is_some();
    let mut scan_roots = env::var_os("RUNES_SCAN_ROOTS").is_some();
    let mut lower_keyword = env::var_os("RUNES_LOWER_KEYWORD").is_some();
    let mut config_path = env::var_os("RUNES_CONFIG").map(std::path::PathBuf::from);
    for arg in env::args() {
        if arg == "--runes-dump-mir" {
            dump_mir = true;
        } else if arg == "--runes-scan-roots" {
            scan_roots = true;
        } else if arg == "--runes-lower-keyword" {
            lower_keyword = true;
        } else if let Some(path) = arg.strip_prefix("--runes-config=") {
            config_path = Some(path.into());
        } else {
            rustc_args.push(arg);
        }
    }

    // Cargo invokes RUSTC_WRAPPER as `wrapper path/to/rustc <rustc args>`.
    // rustc_driver expects the actual compiler path to be argv[0].
    if rustc_args
        .get(1)
        .and_then(|path| std::path::Path::new(path).file_stem())
        .is_some_and(|name| name == "rustc")
    {
        rustc_args.remove(0);
    }

    if rustc_args.len() == 1 {
        eprintln!(
            "Usage: runes-driver [--runes-dump-mir] [--runes-scan-roots] \
             [--runes-lower-keyword] [--runes-config=Runes.toml] <rustc arguments>"
        );
        return ExitCode::FAILURE;
    }

    let syntax = match config_path {
        Some(path) => match SyntaxConfig::load(&path) {
            Ok(syntax) => syntax,
            Err(error) => {
                eprintln!("error: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => SyntaxConfig::default(),
    };
    let mut callbacks = RunesCallbacks {
        dump_mir,
        scan_roots,
        lower_keyword,
        syntax,
    };
    run_compiler(&rustc_args, &mut callbacks);
    ExitCode::SUCCESS
}
