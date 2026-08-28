#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{BTreeSet, HashMap};
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

/// A provenance set: the names of every `rune` root whose data may flow into a
/// MIR local or call. Sorted for deterministic event output.
type Roots = BTreeSet<String>;

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

fn u64_operand<'tcx>(tcx: TyCtxt<'tcx>, value: u64, span: Span) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::from_bits(
            tcx,
            value.into(),
            ty::TypingEnv::fully_monomorphized(),
            tcx.types.u64,
        ),
    }))
}

/// Stable per-site `value_id`: an FNV-1a hash over the source location so the
/// same derive site maps to the same id across runs and builds. This is a
/// placeholder identity until the versioned-value pass assigns runtime ids.
fn site_value_id(file: &str, line: u32, column: u32) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in file.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in line.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in column.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// A comma-separated `&'static str` of root names, or `""` when a value/call
/// has no direct seed. `""` is handled by the runtime as "inherit enclosing
/// frame roots".
fn roots_operand<'tcx>(tcx: TyCtxt<'tcx>, roots: &Roots, span: Span) -> Operand<'tcx> {
    str_operand(tcx, &roots_joined(roots), span)
}

fn roots_joined(roots: &Roots) -> String {
    roots.iter().cloned().collect::<Vec<_>>().join(",")
}

/// Decodes the static `&str` literal emitted by `str_operand` (or the binding
/// argument of `mark_root_at`) back into a `String`.
fn static_str_literal<'tcx>(tcx: TyCtxt<'tcx>, constant: &Const<'tcx>) -> Option<String> {
    let value = constant
        .eval(
            tcx,
            ty::TypingEnv::fully_monomorphized(),
            rustc_span::DUMMY_SP,
        )
        .ok()?;
    let bytes = value.try_get_slice_bytes_for_diagnostics(tcx)?;
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// The binding name recorded by the `mark_root_at` call whose second argument
/// is the `&'static str` constant, if it is statically decodable.
fn root_binding_name<'tcx>(tcx: TyCtxt<'tcx>, func: &Operand<'tcx>, args: &[Spanned<Operand<'tcx>>]) -> Option<String> {
    let (def_id, _) = func.const_fn_def()?;
    let path = tcx.def_path_str(def_id);
    if !path.ends_with("runes_runtime::mark_root") && !path.ends_with("runes_runtime::mark_root_at") {
        return None;
    }
    // `mark_root(value, site)` has no separate binding argument; the binding
    // travels inside `site.binding`. Only the lowered form carries a name.
    if path.ends_with("runes_runtime::mark_root") {
        return None;
    }
    let binding = args.get(1)?;
    let Operand::Constant(constant) = &binding.node else {
        return None;
    };
    static_str_literal(tcx, &constant.const_)
}

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

fn operand_roots(operand: &Operand<'_>, traced: &HashMap<Local, Roots>) -> Roots {
    let Some(place) = operand.place() else {
        return Roots::new();
    };
    traced.get(&place.local).cloned().unwrap_or_default()
}

fn merge_roots(target: &mut Roots, source: &Roots) -> bool {
    let before = target.len();
    target.extend(source.iter().cloned());
    target.len() != before
}

/// Fixed-point provenance propagation over the MIR body.
///
/// `mark_root_at` (and `mark_root`) calls seed their destination local with the
/// decoded binding name; every assignment, call destination, and call argument
/// then merges the union of its source locals' root sets until quiescence.
/// Returns the final root set per local and, for every traced call, its root set.
fn propagate_roots<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
) -> (HashMap<Local, Roots>, HashMap<usize, Roots>) {
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
            let TerminatorKind::Call {
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
    (local_roots, call_roots)
}

#[derive(Debug)]
struct DerivedAssignment {
    block_index: usize,
    statement_index: usize,
    label: String,
    span: Span,
    roots: Roots,
}

fn traced_assignments<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Vec<DerivedAssignment> {
    let (local_roots, _) = propagate_roots(tcx, body);
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
            let roots = rvalue_roots(rvalue, &local_roots);
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

fn instrument_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    enter_hook: DefId,
    exit_hook: DefId,
) {
    let (_, call_roots) = propagate_roots(tcx, body);
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
                            .ends_with("runes_runtime::mark_root_at")
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
