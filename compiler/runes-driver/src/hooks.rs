//! Bridge to the `runes-runtime` hooks the driver injects into MIR.
//!
//! Everything here exists to build operands for the runtime hook calls, or to
//! read data back out of the calls the `rune` lowering produced.

use rustc_middle::mir::interpret::CTFE_ALLOC_SALT;
use rustc_middle::mir::{Const, ConstOperand, ConstValue, Operand};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Spanned};

use crate::provenance::Roots;

/// Resolves a top-level item of a dependency crate, used to locate hooks.
pub fn dependency_item(tcx: TyCtxt<'_>, crate_name: &str, item_name: &str) -> Option<DefId> {
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
                    rustc_hir::def::Res::Def(_, def_id) => Some(def_id),
                    _ => None,
                })
                .flatten()
        })
}

/// A zero-sized constant pointing at a function, used as a call's callee.
pub fn function_operand<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId, span: Span) -> Operand<'tcx> {
    let ty = tcx.type_of(def_id).instantiate_identity().skip_norm_wip();
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::zero_sized(ty),
    }))
}

/// A `&'static str` constant, allocating the bytes in the compiler's arena.
pub fn str_operand<'tcx>(tcx: TyCtxt<'tcx>, value: &str, span: Span) -> Operand<'tcx> {
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

pub fn u32_operand<'tcx>(tcx: TyCtxt<'tcx>, value: u32, span: Span) -> Operand<'tcx> {
    scalar_operand(tcx, value.into(), tcx.types.u32, span)
}

pub fn u64_operand<'tcx>(tcx: TyCtxt<'tcx>, value: u64, span: Span) -> Operand<'tcx> {
    scalar_operand(tcx, value.into(), tcx.types.u64, span)
}

fn scalar_operand<'tcx>(tcx: TyCtxt<'tcx>, value: u128, ty: Ty<'tcx>, span: Span) -> Operand<'tcx> {
    Operand::Constant(Box::new(ConstOperand {
        span,
        user_ty: None,
        const_: Const::from_bits(tcx, value, ty::TypingEnv::fully_monomorphized(), ty),
    }))
}

/// Stable per-site `value_id`: an FNV-1a hash over the source location so the
/// same derive site maps to the same id across runs and builds. This is a
/// placeholder identity until the versioned-value pass assigns runtime ids.
pub fn site_value_id(file: &str, line: u32, column: u32) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in file.as_bytes().iter().chain(&line.to_le_bytes()).chain(&column.to_le_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Comma-separated root names, or `""` for "no direct seed" (the runtime then
/// inherits the enclosing frame's roots).
pub fn roots_operand<'tcx>(tcx: TyCtxt<'tcx>, roots: &Roots, span: Span) -> Operand<'tcx> {
    str_operand(tcx, &roots_joined(roots), span)
}

pub fn roots_joined(roots: &Roots) -> String {
    roots.iter().cloned().collect::<Vec<_>>().join(",")
}

/// Decodes the static `&str` literal produced by `str_operand` back into a
/// `String`, so the driver can read a root binding name out of MIR.
pub fn static_str_literal<'tcx>(tcx: TyCtxt<'tcx>, constant: &Const<'tcx>) -> Option<String> {
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

/// The binding name recorded by a `mark_root_at` call, whose second argument is
/// the `&'static str` constant, if it is statically decodable.
pub fn root_binding_name<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &Operand<'tcx>,
    args: &[Spanned<Operand<'tcx>>],
) -> Option<String> {
    let (def_id, _) = func.const_fn_def()?;
    let path = tcx.def_path_str(def_id);
    // `mark_root(value, site)` carries no separate binding argument, so only
    // the lowered `mark_root_at` form exposes a name.
    if !path.ends_with("runes_runtime::mark_root_at") {
        return None;
    }
    let binding = args.get(1)?;
    let Operand::Constant(constant) = &binding.node else {
        return None;
    };
    static_str_literal(tcx, &constant.const_)
}
