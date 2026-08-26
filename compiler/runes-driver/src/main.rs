#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::env;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use runes_syntax::{lower_rust_source, SyntaxConfig};
use rustc_driver::{run_compiler, Callbacks, Compilation};
use rustc_interface::interface;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::TyCtxt;
use rustc_span::source_map::FileLoader;

#[derive(Default)]
struct RunesCallbacks {
    dump_mir: bool,
    scan_roots: bool,
    lower_keyword: bool,
    syntax: SyntaxConfig,
}

impl Callbacks for RunesCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
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
        lower_rust_source(&source, &self.syntax)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
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
