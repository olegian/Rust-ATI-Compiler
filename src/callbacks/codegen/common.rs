//! Defines helper functions used throughout the code generation process.
//!
//! Almost all generated functions/methods require constructing a string representation of the
//! code, which means we often have to convert from AST nodes to string representations of those
//! nodes. All `*_to_string()` functions address this problem. Further, both functions and methods
//! require some parts that are built in similar ways (e.g. building the input parameters passed
//! to the inner function).
//!
//! Further, each new shim function / method is required to have a unique name within the namespace
//! they are defined in.
//!
//! A few other helpers are defined within this file as well, view individual function doc
//! comments to see what they do.

/// Creates an inner name that does not clash with any other function/method
/// defined in the same `(mod_path, namespace)` slot. `known` is the set of
/// existing fn/method names in that slot, see `FnIndex::names_in`
pub fn get_unique_inner_name(original: &str, known: &std::collections::HashSet<String>) -> String {
    let mut suffix = 0;
    let mut candidate = format!("{original}{suffix}");
    while known.contains(&candidate) {
        suffix += 1;
        candidate = format!("{original}{suffix}");
    }

    candidate
}

/// Converts the generic params to a string like `<'a, T, U: Clone>`.
/// Returns an empty string if there are no generic params.
pub fn generic_params_to_string(generics: &rustc_ast::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }

    let params: Vec<String> = generics
        .params
        .iter()
        .map(|param| match &param.kind {
            rustc_ast::GenericParamKind::Lifetime => {
                let name = param.ident.as_str().to_string();
                if param.bounds.is_empty() {
                    name
                } else {
                    format!(
                        "{}: {}",
                        name,
                        rustc_ast_pretty::pprust::bounds_to_string(&param.bounds)
                    )
                }
            }
            rustc_ast::GenericParamKind::Type { default } => {
                let mut s = param.ident.as_str().to_string();
                if !param.bounds.is_empty() {
                    s.push_str(&format!(
                        ": {}",
                        rustc_ast_pretty::pprust::bounds_to_string(&param.bounds)
                    ));
                }
                if let Some(ty) = default {
                    s.push_str(&format!(
                        " = {}",
                        rustc_ast_pretty::pprust::ty_to_string(ty)
                    ));
                }
                s
            }
            rustc_ast::GenericParamKind::Const { ty, default, .. } => {
                let mut s = format!(
                    "const {}: {}",
                    param.ident.as_str(),
                    rustc_ast_pretty::pprust::ty_to_string(ty)
                );
                if let Some(d) = default {
                    s.push_str(&format!(
                        " = {}",
                        rustc_ast_pretty::pprust::expr_to_string(&d.value)
                    ));
                }
                s
            }
        })
        .collect();

    format!("<{}>", params.join(", "))
}

/// Converts the generic params to a string like `<'a, T, U>` containing only
/// the names of each parameter (no bounds or defaults). Returns an empty
/// string if there are no generic params.
pub fn generic_args_to_string(generics: &rustc_ast::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }

    let args: Vec<String> = generics
        .params
        .iter()
        .map(|param| param.ident.as_str().to_string())
        .collect();

    format!("<{}>", args.join(", "))
}

/// Converts a where clause to a string like ` where T: Clone, U: Send`.
/// Returns an empty string if the where clause is empty.
pub fn where_clause_to_string(generics: &rustc_ast::Generics) -> String {
    if generics.where_clause.predicates.is_empty() {
        return String::new();
    }

    let preds: Vec<String> = generics
        .where_clause
        .predicates
        .iter()
        .map(|pred| match &pred.kind {
            rustc_ast::WherePredicateKind::BoundPredicate(bp) => {
                rustc_ast_pretty::pprust::where_bound_predicate_to_string(bp)
            }
            rustc_ast::WherePredicateKind::RegionPredicate(rp) => {
                let lifetime = format!("'{}", rp.lifetime.ident.as_str());
                if rp.bounds.is_empty() {
                    lifetime
                } else {
                    format!(
                        "{}: {}",
                        lifetime,
                        rustc_ast_pretty::pprust::bounds_to_string(&rp.bounds)
                    )
                }
            }
            rustc_ast::WherePredicateKind::EqPredicate(_) => {
                unreachable!("Found unsupported EqPredicate in where clause")
            }
        })
        .collect();

    format!(" where {}", preds.join(", "))
}

/// gets the name of a parameter passed to some function
// FIXME: I'm not sure why using pprust::pat_to_string(param.pat) instead causes a panic?
pub fn get_param_name(param: &rustc_ast::Param) -> String {
    match param.pat.kind {
        rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
        _ => unreachable!("Cannot get name of non-Ident param name"),
    }
}

/// Source for the inner-fn argument list: each TaggedRefMut formal forwards
/// as `name.reborrow()`; everything else forwards as `name`.
pub fn build_inner_call_args<'a>(params: impl Iterator<Item = &'a rustc_ast::Param>) -> String {
    params
        .map(|p| {
            let name = get_param_name(p);
            if is_tagged_ref_mut(&p.ty) {
                format!("{name}.reborrow()")
            } else {
                name
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Generates bind statements for parameters against a site variable.
///
/// Skips any formal whose `VariableDecl` in `ppt` is tagged
/// `constant UNINITIALIZED`. At ENTER sites this is a no-op since formals
/// have not yet been moved/dropped. At EXIT sites this drops the dead
/// formals so we don't read moved-out values.
pub fn create_param_binds<'a>(
    site_name: &str,
    params: impl Iterator<Item = &'a rustc_ast::Param>,
    ppt: &decls_gen::ProgramPoint,
) -> Vec<String> {
    params
        .filter(|param| {
            matches!(
                &param.ty.kind,
                rustc_ast::TyKind::Array(_, _)
                    | rustc_ast::TyKind::Slice(_)
                    | rustc_ast::TyKind::Ref(_, _)
                    | rustc_ast::TyKind::Tup(_)
                    | rustc_ast::TyKind::Path(_, _)
            )
        })
        .filter_map(|param| {
            let var_name = get_param_name(param);
            if is_dead(ppt, &var_name) {
                return None;
            }
            Some(format!(
                r#"{var_name}.bind(&mut {site_name}, "{var_name}");"#
            ))
        })
        .collect()
}

/// Returns true iff `ppt`'s `VariableDecl` for `formal` is tagged
/// `constant UNINITIALIZED`. Panics if the formal is missing from the ppt.
pub fn is_dead(ppt: &decls_gen::ProgramPoint, formal: &str) -> bool {
    ppt.var_decl(formal.to_string())
        .unwrap_or_else(|| {
            panic!(
                "stub generation: ppt is missing VariableDecl for formal `{formal}`, \
                 pass 1 should have rejected this; DATIR/decls-gen drift?"
            )
        })
        .is_uninit()
}

/// True if `ty`'s outer wrapper is `TaggedRefMut<...>`. Used by the wrapper
/// to decide whether the formal needs a `.reborrow()` when forwarded to the
/// inner fn. `TaggedRefMut` is move-only, but the binding still has to live
/// for the EXIT-site binds.
fn is_tagged_ref_mut(ty: &rustc_ast::Ty) -> bool {
    let rustc_ast::TyKind::Path(_, path) = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .map(|seg| seg.ident.name.as_str() == "TaggedRefMut")
        .unwrap_or(false)
}
