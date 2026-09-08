//! `runes-driver`: a `rustc_driver` that instruments Runes-traced code.
//!
//! Invoked as a `RUSTC_WRAPPER` (usually by `cargo runes`). It lowers `rune`
//! root declarations on the fly, then overrides the `optimized_mir` query to
//! propagate shadow provenance and insert runtime hooks.

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

mod hooks;
mod instrument;
mod provenance;

use std::env;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use runes_syntax::{SyntaxConfig, lower_rust_source};
use rustc_driver::{Callbacks, Compilation, run_compiler};
use rustc_interface::interface;
use rustc_middle::mir::{Body, TerminatorKind};
use rustc_middle::ty::TyCtxt;
use rustc_span::def_id::LocalDefId;
use rustc_span::source_map::FileLoader;

use provenance::propagate_roots;

type OptimizedMirProvider = for<'tcx> fn(TyCtxt<'tcx>, LocalDefId) -> &'tcx Body<'tcx>;

static ORIGINAL_OPTIMIZED_MIR: std::sync::OnceLock<OptimizedMirProvider> =
    std::sync::OnceLock::new();
static HAS_RUNE_SOURCE: AtomicBool = AtomicBool::new(false);

/// Overrides `optimized_mir` so every body is analysed and instrumented once,
/// before codegen sees it.
fn runes_optimized_mir<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> &'tcx Body<'tcx> {
    let original = ORIGINAL_OPTIMIZED_MIR
        .get()
        .expect("Runes optimized_mir provider was not installed");
    let mut body = original(tcx, def_id).clone();
    if HAS_RUNE_SOURCE.load(Ordering::Relaxed) && instrument::resolve_hooks(tcx).is_some() {
        let provenance = propagate_roots(tcx, &body);
        instrument::instrument_assignments(tcx, &mut body, &provenance.local_roots);
        instrument::instrument_calls(tcx, &mut body, &provenance.call_roots);
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

/// Driver options, from the environment or `--runes-*` flags.
#[derive(Default)]
struct Options {
    dump_mir: bool,
    scan_roots: bool,
    lower_keyword: bool,
    config: Option<std::path::PathBuf>,
}

fn main() -> ExitCode {
    let mut rustc_args = Vec::new();
    let mut options = Options {
        dump_mir: env::var_os("RUNES_DUMP_MIR").is_some(),
        scan_roots: env::var_os("RUNES_SCAN_ROOTS").is_some(),
        lower_keyword: env::var_os("RUNES_LOWER_KEYWORD").is_some(),
        config: env::var_os("RUNES_CONFIG").map(std::path::PathBuf::from),
    };

    for arg in env::args() {
        match arg.as_str() {
            "--runes-dump-mir" => options.dump_mir = true,
            "--runes-scan-roots" => options.scan_roots = true,
            "--runes-lower-keyword" => options.lower_keyword = true,
            _ => {
                if let Some(path) = arg.strip_prefix("--runes-config=") {
                    options.config = Some(path.into());
                } else {
                    rustc_args.push(arg);
                }
            }
        }
    }

    // Cargo invokes RUSTC_WRAPPER as `wrapper path/to/rustc <rustc args>`;
    // rustc_driver expects the real compiler path at argv[0].
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

    let syntax = match options.config {
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
        dump_mir: options.dump_mir,
        scan_roots: options.scan_roots,
        lower_keyword: options.lower_keyword,
        syntax,
    };
    run_compiler(&rustc_args, &mut callbacks);
    ExitCode::SUCCESS
}
