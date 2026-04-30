/* Walks the already-transformed AST to generate function stubs and BindToSite impls
 * to perform ENTER/EXIT site management.
 *
 * Importantly, all instrumented functions are renamed to a new "inner" name,
 * the generated stub will be called the original function name, and will invoke
 * this inner function. Name conflicts are avoided by scanning over all user-defined
 * functions and methods, and then finding a suffix to append to the inner name
 * that makes it unique.
*/
use decls_gen::decls::RETURN_VAR_NAME;
use decls_gen::ProgramPoint;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::{self as ast};
use rustc_ast_pretty::pprust;
use rustc_span::symbol::kw;

use crate::common::{self, DatirConfig, parsing};
use crate::types::ati_info::{FirstPassInfo, TypeKey};

// FIXME: time for another rewrite.. this is unruly
// probably split into a file for fns, a file for methods,
// and a file for helpers...

/// Describes how a method receives its `self` argument
#[derive(Debug)]
pub enum ReceiverKind {
    /// Associated function with no self parameter.
    None,
    /// Takes self
    Value,
    /// Takes &self
    Ref,
    /// Takes &mut self
    RefMut,
}

/// For each fn/method in the crate (recursing into inline submodules),
/// modifies the body of the function to create enter and exit sites
/// with input parameters bound to each. The new body will then invoke
/// an "inner" function, which holds the actual function logic. This
/// inner function is also generated, with a non-colliding name and
/// matching trait constraints/generics.
///
/// For each user-defined compound types, also generates a SiteBind impl,
/// to allow associating the compound type's leaves with a site.
///
/// module_path is the file-derived Rust module path ("" for the crate
/// root, "dep" for a non-root file).
pub fn generate_stubs(
    datir_config: &DatirConfig,
    first_pass: &FirstPassInfo,
    krate: &mut ast::Crate,
    module_path: &str,
    psess: &rustc_session::parse::ParseSess,
) {
    generate_stubs_in_mod(
        datir_config,
        first_pass,
        &mut krate.items,
        module_path,
        psess,
    );
}

/// Recursive worker for generate_stubs. Walks one module's items at
/// mod_path, recursing into ItemKind::Mod with mod_path::sub_name.
fn generate_stubs_in_mod(
    datir_config: &DatirConfig,
    first_pass: &FirstPassInfo,
    items: &mut thin_vec::ThinVec<Box<ast::Item>>,
    mod_path: &str,
    psess: &rustc_session::parse::ParseSess,
) {
    // inner fns and the generated inherent impl blocks holding inner methods
    let mut new_items: Vec<Box<ast::Item>> = Vec::new();

    // Compound type SiteBind impls
    let mut bind_impl_code: Vec<String> = Vec::new();

    for item in items.iter_mut() {
        match &mut item.kind {
            // Free functions
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                generics,
                sig: ast::FnSig { decl, .. },
                body,
                ..
            }) => {
                // find a name for the function which does not conflict with
                // any other name in the current module namespace.
                let orig_name = ident.as_str().to_string();
                let known_names = first_pass.known_fn_names_in(mod_path, None);
                let inner_name = get_unique_inner_name(&orig_name, &known_names);
                if datir_config.print_function_signatures {
                    datir_config.log(
                        "FunctionStubs",
                        format!(
                            "Generating free function stub for {mod_path}: {:?} -> {:?}",
                            orig_name, inner_name
                        ),
                    );
                }

                // find the name of the base program point name, discovered in the first pass
                let entry = first_pass
                    .lookup_free_fn(mod_path, ident.name)
                    .unwrap_or_else(|| {
                        panic!(
                            "stub generation could not find a FnEntry for free fn \
                             `{orig_name}` in module `{mod_path}`"
                        )
                    });

                // rip the original body out of the free function.
                let orig_body = body.take().unwrap_or_else(|| {
                    panic!(
                        "free fn `{orig_name}` in module `{mod_path}` has no body."
                    )
                });

                // construct fn item that will at some point contain the original body
                let inner_template =
                    build_inner_fn_template(&inner_name, generics, &decl.inputs, &decl.output);
                let mut parsed_items = common::parse_items(psess, inner_template, None);
                let mut inner_item = parsed_items
                    .pop()
                    .expect("inner-fn template did not parse into an item");

                // get a mutable reference to the empty body and place the original
                // body in there!
                let ast::ItemKind::Fn(box ast::Fn {
                    body: ref mut inner_body,
                    ..
                }) = inner_item.kind
                else {
                    panic!("inner-fn template did not yield ItemKind::Fn");
                };
                *inner_body = Some(orig_body);

                // we will have to add this inner function item into the current
                // module
                new_items.push(inner_item);

                // EXIT-ppt liveness will determine which formals get bound at the
                // exit site. Pass 1 already validated existence of both ppts.
                let enter_ppt = datir_config
                    .decls_file
                    .enter_ppt(&entry.base_ppt_name)
                    .expect("ENTER ppt missing.");
                let exit_ppt = datir_config
                    .decls_file
                    .exit_ppt(&entry.base_ppt_name)
                    .expect("EXIT ppt missing");

                // construct the "stub code", and insert it where the original body was.
                let wrapper_src = build_fn_wrapper_block(
                    datir_config,
                    &entry.base_ppt_name,
                    &orig_name,
                    &inner_name,
                    &decl.inputs,
                    &decl.output,
                    enter_ppt,
                    exit_ppt,
                );
                let parsed_wrapper = common::parse_expr(psess, wrapper_src);
                let ast::ExprKind::Block(new_block, _) = parsed_wrapper.kind else {
                    panic!(
                        "wrapper-block source for free fn `{orig_name}` did not parse as a block"
                    );
                };
                *body = Some(new_block);
            }

            // Struct definition, emit BindToSite impl.
            ast::ItemKind::Struct(ident, generics, ast::VariantData::Struct { fields, .. })
            | ast::ItemKind::Struct(ident, generics, ast::VariantData::Tuple(fields, ..)) => {
                bind_impl_code.push(create_struct_bind_impl(ident.as_str(), fields, generics));
            }

            // Enum definition, emit BindToSite impl.
            ast::ItemKind::Enum(ident, generics, ast::EnumDef { variants }) => {
                bind_impl_code.push(create_enum_bind_impl(ident.as_str(), variants, generics));
            }

            // Impl block come in two flavors, inherent and trait-based.
            // we treat them very similary though! Same strategy to replace
            // the existing body with a stub, but we then create a whole impl
            // block to house the inner method. It's important to carry across
            // the where clause, and any introduced generics into the new impl.
            ast::ItemKind::Impl(ast::Impl {
                generics: impl_generics,
                of_trait,
                self_ty,
                items: impl_items,
                ..
            }) => {
                // split apart the impl block components
                let type_name = pprust::ty_to_string(self_ty);
                let type_key =
                    ast_impl_type_key(of_trait.as_deref().map(|h| &h.trait_ref), self_ty)
                        .unwrap_or_else(|| {
                            panic!(
                                "stub generation could not derive TypeKey from impl self-type \
                             `{type_name}` in module `{mod_path}`; only path self/trait \
                             types are supported"
                            )
                        });
                let impl_generic_params = generic_params_to_string(impl_generics);
                let impl_where_clause = where_clause_to_string(impl_generics);

                // traits can have associated types!! if any are defined
                // and used within the original body, we need to change them
                // to be fully qualified (e.g. <Self as Add>::Output).
                // Given that we need to do this for all associated functions,
                // extract the list of segments in the trait early
                let trait_segs: Option<thin_vec::ThinVec<ast::PathSegment>> = of_trait
                    .as_deref()
                    .map(|h| h.trait_ref.path.segments.clone());

                // all items in this impl will be in this namespace
                let known_names = first_pass.known_fn_names_in(mod_path, Some(&type_key));

                let mut inner_templates: Vec<String> = Vec::new();
                let mut taken_bodies = Vec::new();

                for assoc_item in impl_items.iter_mut() {
                    // we only care about functions in impl blocks.
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        generics: method_generics,
                        sig: ast::FnSig { decl, .. },
                        body,
                        ..
                    }) = &mut assoc_item.kind
                    else {
                        continue;
                    };

                    // same strategy here as free functions.
                    let orig_name = ident.as_str().to_string();
                    let inner_name = get_unique_inner_name(&orig_name, &known_names);
                    if datir_config.print_function_signatures {
                        datir_config.log(
                            "FunctionStubs",
                            format!(
                                "Generating method stub for {mod_path}::{type_key}: {:?} -> {:?}",
                                orig_name, inner_name
                            ),
                        );
                    }

                    let entry = first_pass
                        .lookup_method(mod_path, &type_key, ident.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "stub generation could not find a FnEntry for method \
                                 `{type_key}::{orig_name}` in module `{mod_path}`; \
                                 pass 1 should have populated FirstPassInfo for every \
                                 tracked method"
                            )
                        });

                    let mut orig_body = body.take().unwrap_or_else(|| {
                        panic!(
                            "method `{type_key}::{orig_name}` in module `{mod_path}` has \
                             no body - cannot move it into the inner method"
                        )
                    });

                    // as mentioned above, we have to qualify all associated type
                    // usages in inputs/outputs/body of generated code
                    if let Some(segs) = trait_segs.as_deref() {
                        let mut qualifier = SelfPathQualifier { trait_segs: segs };

                        // rewrite any usages in the method signature
                        for param in decl.inputs.iter_mut() {
                            qualifier.visit_ty(&mut param.ty);
                        }
                        if let ast::FnRetTy::Ty(ret_ty) = &mut decl.output {
                            qualifier.visit_ty(ret_ty);
                        }

                        // rewrite any usages in the body
                        qualifier.visit_block(&mut orig_body);
                    }

                    inner_templates.push(build_inner_method_template(
                        &inner_name,
                        method_generics,
                        &decl.inputs,
                        &decl.output,
                    ));
                    taken_bodies.push(orig_body);

                    // EXIT-ppt liveness will determine which formals get bound at
                    // the exit site (e.g. owned `self` is dead at exit unless Copy).
                    let enter_ppt = datir_config
                        .decls_file
                        .enter_ppt(&entry.base_ppt_name)
                        .expect("ENTER ppt missing — should have been validated in pass 1");
                    let exit_ppt = datir_config
                        .decls_file
                        .exit_ppt(&entry.base_ppt_name)
                        .expect("EXIT ppt missing — should have been validated in pass 1");

                    // replace the existing method body with the stub code,
                    // the code that creates ENTER/EXIT points, and calls
                    // the inner function inside of it.
                    let wrapper_src = build_method_wrapper_block(
                        &entry.base_ppt_name,
                        &inner_name,
                        &decl.inputs,
                        &decl.output,
                        enter_ppt,
                        exit_ppt,
                    );
                    let parsed_wrapper = common::parse_expr(psess, wrapper_src);
                    let ast::ExprKind::Block(new_block, _) = parsed_wrapper.kind else {
                        panic!(
                            "wrapper-block source for method `{type_key}::{orig_name}` \
                             did not parse as a block"
                        );
                    };
                    *body = Some(new_block);
                }

                if !inner_templates.is_empty() {
                    // if we have something to add, unify every method defined in the current impl
                    // block into another impl block with the same generics / where clause
                    let impl_template = format!(
                        "impl{impl_generic_params} {type_name}{impl_where_clause} {{\n{}\n}}",
                        inner_templates.join("\n\n"),
                    );

                    let mut parsed_items = common::parse_items(psess, impl_template, None);
                    let mut impl_item = parsed_items
                        .pop()
                        .expect("inner-impl template did not parse into an item");
                    let ast::ItemKind::Impl(ast::Impl {
                        items: ref mut parsed_assoc,
                        ..
                    }) = impl_item.kind
                    else {
                        panic!("inner-impl template did not yield ItemKind::Impl");
                    };

                    if parsed_assoc.len() != taken_bodies.len() {
                        panic!(
                            "inner-impl assoc count ({}) != taken body count ({}) for \
                             `{type_key}` in `{mod_path}`",
                            parsed_assoc.len(),
                            taken_bodies.len(),
                        );
                    }

                    for (assoc, orig_body) in parsed_assoc.iter_mut().zip(taken_bodies) {
                        let ast::AssocItemKind::Fn(box ast::Fn {
                            body: ref mut inner_body,
                            ..
                        }) = assoc.kind
                        else {
                            panic!("parsed inner method was not a Fn");
                        };
                        *inner_body = Some(orig_body);
                    }

                    // add whole impl to the current mod
                    new_items.push(impl_item);
                }
            }

            // Recurse into submodules, update mod path.
            ast::ItemKind::Mod(_, mod_ident, ast::ModKind::Loaded(sub_items, _, _)) => {
                let sub_mod_path = if mod_path.is_empty() {
                    mod_ident.as_str().to_string()
                } else {
                    format!("{mod_path}::{}", mod_ident.as_str())
                };
                generate_stubs_in_mod(datir_config, first_pass, sub_items, &sub_mod_path, psess);
            }

            _ => {}
        }
    }

    // Append inner items (inner fns + generated inherent impl blocks).
    for new_item in new_items {
        items.push(new_item);
    }

    // Add struct/enum SiteBind impls.
    for code in bind_impl_code {
        for parsed_item in parsing::parse_items(psess, code, None) {
            items.push(parsed_item);
        }
    }

    if !mod_path.is_empty() {
        // if we are in a submodule / non-root file, then import root
        // to make types available.
        let imports = common::parse_items(psess, "use crate::*;".into(), None);
        for import in imports {
            items.insert(0, import);
        }
    }
}

/// Creates a TypeKey for an impl block, derived from its `self_ty` and `of_trait`.
///
/// Returns `None` when either path can't be canonicalized.
/// Mirrors gather_orig.rs::impl_type_key so both pass-1 and pass-2 produce identical keys.
pub fn ast_impl_type_key(of_trait: Option<&ast::TraitRef>, self_ty: &ast::Ty) -> Option<TypeKey> {
    let self_path = ast_ty_canonical(self_ty)?;
    let trait_path = match of_trait {
        Some(tr) => Some(ast_path_canonical(&tr.path)?),
        None => None,
    };
    Some(match trait_path {
        Some(t) => TypeKey::trait_impl(self_path, t),
        None => TypeKey::inherent(self_path),
    })
}

/// Canonical ::-joined string form of an AST path.
/// Mirrors `gather_orig.rs::hir_path_canonical`.
fn ast_path_canonical(path: &ast::Path) -> Option<String> {
    let mut parts = Vec::with_capacity(path.segments.len());
    for seg in path.segments.iter() {
        parts.push(ast_segment_canonical(seg)?);
    }
    Some(parts.join("::"))
}

// Canonicalizes a single segment of a path.
fn ast_segment_canonical(seg: &ast::PathSegment) -> Option<String> {
    let ident = seg.ident.name.to_string();
    let Some(args) = &seg.args else {
        return Some(ident);
    };
    let ast::GenericArgs::AngleBracketed(args) = args.as_ref() else {
        return None;
    };

    let mut rendered = Vec::new();
    for arg in args.args.iter() {
        let ast::AngleBracketedArg::Arg(generic_arg) = arg else {
            return None;
        };
        let s = match generic_arg {
            ast::GenericArg::Lifetime(lt) => lt.ident.name.to_string(),
            ast::GenericArg::Type(ty) => ast_ty_canonical(ty)?,
            ast::GenericArg::Const(_) => panic!(
                "DATIR does not support const generic arguments in impl-block paths \
                 (encountered in segment `{}`); see ast_segment_canonical",
                seg.ident.name
            ),
        };
        rendered.push(s);
    }

    if rendered.is_empty() {
        Some(ident)
    } else {
        Some(format!("{ident}<{}>", rendered.join(",")))
    }
}

// Canonicalizes a Path type name
fn ast_ty_canonical(ty: &ast::Ty) -> Option<String> {
    if matches!(ty.kind, ast::TyKind::Infer) {
        panic!(
            "DATIR does not support inferred (`_`) generic arguments in impl-block \
             paths; see ast_ty_canonical"
        );
    }
    let ast::TyKind::Path(_, path) = &ty.kind else {
        return None;
    };
    ast_path_canonical(path)
}

/// Converts the generic params to a string like `<'a, T, U: Clone>`.
/// Returns an empty string if there are no generic params.
fn generic_params_to_string(generics: &ast::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }

    let params: Vec<String> = generics
        .params
        .iter()
        .map(|param| match &param.kind {
            ast::GenericParamKind::Lifetime => {
                let name = param.ident.as_str().to_string();
                if param.bounds.is_empty() {
                    name
                } else {
                    format!("{}: {}", name, pprust::bounds_to_string(&param.bounds))
                }
            }
            ast::GenericParamKind::Type { default } => {
                let mut s = param.ident.as_str().to_string();
                if !param.bounds.is_empty() {
                    s.push_str(&format!(": {}", pprust::bounds_to_string(&param.bounds)));
                }
                if let Some(ty) = default {
                    s.push_str(&format!(" = {}", pprust::ty_to_string(ty)));
                }
                s
            }
            ast::GenericParamKind::Const { ty, default, .. } => {
                let mut s = format!(
                    "const {}: {}",
                    param.ident.as_str(),
                    pprust::ty_to_string(ty)
                );
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", pprust::expr_to_string(&d.value)));
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
fn generic_args_to_string(generics: &ast::Generics) -> String {
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
fn where_clause_to_string(generics: &ast::Generics) -> String {
    if generics.where_clause.predicates.is_empty() {
        return String::new();
    }

    let preds: Vec<String> = generics
        .where_clause
        .predicates
        .iter()
        .map(|pred| match &pred.kind {
            ast::WherePredicateKind::BoundPredicate(bp) => {
                pprust::where_bound_predicate_to_string(bp)
            }
            ast::WherePredicateKind::RegionPredicate(rp) => {
                let lifetime = format!("'{}", rp.lifetime.ident.as_str());
                if rp.bounds.is_empty() {
                    lifetime
                } else {
                    format!("{}: {}", lifetime, pprust::bounds_to_string(&rp.bounds))
                }
            }
            ast::WherePredicateKind::EqPredicate(ep) => {
                unreachable!("Found unsupported EqPredicate in where clause")
            }
        })
        .collect();

    format!(" where {}", preds.join(", "))
}

/// Creates an inner name that does not clash with any other function/method
/// defined in the same `(mod_path, namespace)` slot. `known` is the set of
/// existing fn/method names in that slot, see `FirstPassInfo::known_fn_names_in`
fn get_unique_inner_name(original: &str, known: &std::collections::HashSet<String>) -> String {
    let mut suffix = 0;
    let mut candidate = format!("{original}{suffix}");
    while known.contains(&candidate) {
        suffix += 1;
        candidate = format!("{original}{suffix}");
    }

    candidate
}

/// Determines whether a list of parameters being passed to some
/// function or method accepts self, &self, &mut self.
fn determine_receiver_kind(params: &[ast::Param]) -> ReceiverKind {
    // can the self parameter be something other than the first param?
    let Some(first) = params.first() else {
        return ReceiverKind::None;
    };

    match &first.ty.kind {
        ast::TyKind::ImplicitSelf => ReceiverKind::Value,
        ast::TyKind::Ref(_, ast::MutTy { mutbl, .. }) => {
            if matches!(mutbl, ast::Mutability::Mut) {
                ReceiverKind::RefMut
            } else {
                ReceiverKind::Ref
            }
        }
        _ => ReceiverKind::None,
    }
}

/// gets the name of a parameter passed to some function
// FIXME: I'm not sure why using pprust::pat_to_string(param.pat) instead causes a panic?
fn get_param_name(param: &ast::Param) -> String {
    match param.pat.kind {
        rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
        _ => unreachable!("Cannot get name of non-Ident param name"),
    }
}

/// True if `ty`'s outer wrapper is `TaggedRefMut<...>`. Used by the wrapper
/// to decide whether the formal needs a `.reborrow()` when forwarded to the
/// inner fn — `TaggedRefMut` is move-only, but the binding still has to live
/// for the EXIT-site binds.
fn is_tagged_ref_mut(ty: &ast::Ty) -> bool {
    let ast::TyKind::Path(_, path) = &ty.kind else {
        return false;
    };
    path.segments
        .last()
        .map(|seg| seg.ident.name.as_str() == "TaggedRefMut")
        .unwrap_or(false)
}

/// Source for the inner-fn argument list: each TaggedRefMut formal forwards
/// as `name.reborrow()`; everything else forwards as `name`.
fn build_inner_call_args<'a>(params: impl Iterator<Item = &'a ast::Param>) -> String {
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
fn create_param_binds<'a>(
    site_name: &str,
    params: impl Iterator<Item = &'a ast::Param>,
    ppt: &ProgramPoint,
) -> Vec<String> {
    params
        .filter(|param| {
            matches!(
                &param.ty.kind,
                ast::TyKind::Array(_, _)
                    | ast::TyKind::Slice(_)
                    | ast::TyKind::Ref(_, _)
                    | ast::TyKind::Tup(_)
                    | ast::TyKind::Path(_, _)
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
fn is_dead(ppt: &ProgramPoint, formal: &str) -> bool {
    ppt.var_decl(formal.to_string())
        .unwrap_or_else(|| {
            panic!(
                "stub generation: ppt is missing VariableDecl for formal `{formal}` \
                 — pass 1 should have rejected this; DATIR/decls-gen drift?"
            )
        })
        .is_uninit()
}

// ========== Self-path qualifier ==========

/// In-place mut visitor that rewrites every Self::X path in
/// signature types, body expressions, and body patterns into the
/// fully-qualified form <Self as Trait>::X.
struct SelfPathQualifier<'a> {
    trait_segs: &'a [ast::PathSegment],
}

impl<'a> SelfPathQualifier<'a> {
    // rewrite a path segment, if it access an associated type via Self
    fn maybe_rewrite(&self, qself: &mut Option<Box<ast::QSelf>>, path: &mut ast::Path) {
        if qself.is_some() || path.segments.len() < 2 {
            return;
        }
        if path.segments[0].ident.name != kw::SelfUpper {
            return;
        }

        let tail: thin_vec::ThinVec<ast::PathSegment> =
            path.segments.iter().skip(1).cloned().collect();

        let mut new_segs: thin_vec::ThinVec<ast::PathSegment> =
            self.trait_segs.iter().cloned().collect();

        let position = new_segs.len();
        new_segs.extend(tail);
        path.segments = new_segs;

        let self_ty = Box::new(ast::Ty {
            id: ast::DUMMY_NODE_ID,
            kind: ast::TyKind::Path(
                None,
                ast::Path {
                    span: rustc_span::DUMMY_SP,
                    segments: thin_vec::thin_vec![ast::PathSegment {
                        ident: rustc_span::Ident::with_dummy_span(kw::SelfUpper),
                        id: ast::DUMMY_NODE_ID,
                        args: None,
                    }],
                    tokens: None,
                },
            ),
            span: rustc_span::DUMMY_SP,
            tokens: None,
        });
        *qself = Some(Box::new(ast::QSelf {
            ty: self_ty,
            path_span: rustc_span::DUMMY_SP,
            position,
        }));
    }
}

/// Self paths could be in types, expressions, or patterns.
/// Make sure to visit all of them.
impl<'a> MutVisitor for SelfPathQualifier<'a> {
    fn visit_ty(&mut self, ty: &mut ast::Ty) {
        if let ast::TyKind::Path(qself, path) = &mut ty.kind {
            self.maybe_rewrite(qself, path);
        }
        mut_visit::walk_ty(self, ty);
    }

    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        if let ast::ExprKind::Path(qself, path) = &mut expr.kind {
            self.maybe_rewrite(qself, path);
        }
        mut_visit::walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: &mut ast::Pat) {
        if let ast::PatKind::Path(qself, path) = &mut pat.kind {
            self.maybe_rewrite(qself, path);
        }
        mut_visit::walk_pat(self, pat);
    }
}

////////////////////// Wrapper / Inner Builders //////////////////////////

/// Body for a free fn's wrapper, in the single-exit-site flow:
/// 1. open ENTER, bind every formal, update;
/// 2. call `inner_name(args)`;
/// 3. open EXIT, bind only formals still live at exit (per the EXIT ppt's
///    `is_uninit()` tags) and the return value when non-unit, update.
///
/// Special-cased for fn_name == "main": no param binds, no return value to
/// bind, and the analysis report is produced after the EXIT site update.
fn build_fn_wrapper_block(
    config: &DatirConfig,
    base_ppt_name: &str,
    fn_name: &str,
    inner_name: &str,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
    enter_ppt: &ProgramPoint,
    exit_ppt: &ProgramPoint,
) -> String {
    let passed = build_inner_call_args(inputs.iter());
    let enter_binds = create_param_binds("site_enter", inputs.iter(), enter_ppt).join("\n");
    let exit_binds = create_param_binds("site_exit", inputs.iter(), exit_ppt).join("\n");

    if fn_name == "main" {
        // In --release mode each execution produces a fresh .ati file at
        // {ati_output_dir}/{rand:016x}.ati so concurrent or repeated runs
        // don't clobber each other. The dir was wiped + canonicalized in
        // datir's main.rs, so the path here is absolute. Outside release
        // mode just dump the report to stdout via the existing API.
        let post = match &config.ati_output_dir {
            Some(dir) => format!(
                r#"{{
                    let __ati_rand: u64 = std::random::random(..);
                    let __ati_path = format!(r"{}/{{:016x}}.ati", __ati_rand);
                    ATI_ANALYSIS.lock().unwrap().produce_ati(&__ati_path);
                }}"#,
                dir.to_str().expect("ati_output_dir is not valid UTF-8"),
            ),
            None => "ATI_ANALYSIS.lock().unwrap().report();".to_string(),
        };

        return format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {inner_name}();

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                {post}
            }}"#
        );
    }

    match output {
        ast::FnRetTy::Ty(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let res = {inner_name}({passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                res.bind(&mut site_exit, "{RETURN_VAR_NAME}");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                return res;
            }}"#
        ),
        ast::FnRetTy::Default(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {inner_name}({passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
            }}"#
        ),
    }
}

/// Similar to `build_fn_wrapper_block` but for methods.
/// A small difference: the self parameter needs to be correctly
/// used when invoking the inner function. The receiver is bound under the
/// name `"self"`, and is filtered by liveness in the same way as other
/// formals (an owned non-Copy `self` is dead at exit and gets skipped).
fn build_method_wrapper_block(
    base_ppt_name: &str,
    inner_name: &str,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
    enter_ppt: &ProgramPoint,
    exit_ppt: &ProgramPoint,
) -> String {
    let receiver = determine_receiver_kind(inputs);

    let mut inputs_iter = inputs.iter();
    if !matches!(receiver, ReceiverKind::None) {
        inputs_iter.next();
    }
    let non_self: Vec<&ast::Param> = inputs_iter.collect();

    let passed = build_inner_call_args(non_self.iter().copied());

    let call_expr = match receiver {
        ReceiverKind::None => format!("Self::{inner_name}({passed})"),
        _ => format!("self.{inner_name}({passed})"),
    };

    let self_bind = |site_name: &str, ppt: &ProgramPoint| -> String {
        if matches!(receiver, ReceiverKind::None) || is_dead(ppt, "self") {
            return String::new();
        }
        match receiver {
            ReceiverKind::None => unreachable!(),
            ReceiverKind::Value => format!(r#"self.bind(&mut {site_name}, "self");"#),
            ReceiverKind::Ref | ReceiverKind::RefMut => {
                format!(r#"(*self).bind(&mut {site_name}, "self");"#)
            }
        }
    };

    let enter_binds = std::iter::once(self_bind("site_enter", enter_ppt))
        .chain(create_param_binds(
            "site_enter",
            non_self.iter().copied(),
            enter_ppt,
        ))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let exit_binds = std::iter::once(self_bind("site_exit", exit_ppt))
        .chain(create_param_binds(
            "site_exit",
            non_self.iter().copied(),
            exit_ppt,
        ))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    match output {
        ast::FnRetTy::Ty(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let res = {call_expr};

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                res.bind(&mut site_exit, "{RETURN_VAR_NAME}");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                return res;
            }}"#
        ),
        ast::FnRetTy::Default(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {call_expr};

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
            }}"#
        ),
    }
}

/// Source for an inner free function, the signature with an empty placeholder
/// body. The caller parses this template, then transplants the user's
/// original body.
fn build_inner_fn_template(
    inner_name: &str,
    generics: &ast::Generics,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
) -> String {
    let generic_params = generic_params_to_string(generics);
    let where_clause = where_clause_to_string(generics);
    let declared = inputs
        .iter()
        .map(|p| {
            // if p was a mutable ref, we had to make sure the emitted
            // formal is declared to be a mut binding... p was translated to
            // be a TaggedRefMut in this case... But also any struct that carried a mut ref
            // also needs to allow mutable access!! just make everything mutable?
            // FIXME: i honestly think this system sucks. not sure how to avoid it
            // without changing the ref (specifically mut ref) implementation again...
            format!("mut {}: {}", get_param_name(p), pprust::ty_to_string(&p.ty))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret = match output {
        ast::FnRetTy::Ty(t) => format!(" -> {}", pprust::ty_to_string(t)),
        ast::FnRetTy::Default(_) => String::new(),
    };
    format!("fn {inner_name}{generic_params}({declared}){ret}{where_clause} {{ }}")
}

/// Very similar to `build_inner_fn_template`, but for methods.
fn build_inner_method_template(
    inner_name: &str,
    method_generics: &ast::Generics,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
) -> String {
    let generic_params = generic_params_to_string(method_generics);
    let where_clause = where_clause_to_string(method_generics);

    let receiver = determine_receiver_kind(inputs);
    let mut iter = inputs.iter();
    let receiver_str = match receiver {
        ReceiverKind::None => "",
        ReceiverKind::Value => {
            iter.next();
            "self"
        }
        ReceiverKind::Ref => {
            iter.next();
            "&self"
        }
        ReceiverKind::RefMut => {
            iter.next();
            "&mut self"
        }
    };
    let other =
        iter // FIXME: if the above FIXME is addressed, this also needs to change
            .map(|p| format!("mut {}: {}", get_param_name(p), pprust::ty_to_string(&p.ty)))
            .collect::<Vec<_>>()
            .join(", ");
    let declared = match (receiver_str.is_empty(), other.is_empty()) {
        (true, _) => other,
        (false, true) => receiver_str.to_string(),
        (false, false) => format!("{receiver_str}, {other}"),
    };
    let ret = match output {
        ast::FnRetTy::Ty(t) => format!(" -> {}", pprust::ty_to_string(t)),
        ast::FnRetTy::Default(_) => String::new(),
    };
    format!("fn {inner_name}{generic_params}({declared}){ret}{where_clause} {{ }}")
}

/// Implements the BindToSite trait (defined in the runtime library) on some struct.
///
/// This allows calling struct.bind(site) to recursively associated all fields of the
/// struct with the passed in site. Stub functions rely on this to add all relevant
/// values to sites.
fn create_struct_bind_impl(
    struct_name: &str,
    fields: &[ast::FieldDef],
    generics: &ast::Generics,
) -> String {
    let bind_calls = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let stmt = match &field.ident {
                Some(field_name) => {
                    let field_name = field_name.as_str();
                    format!(
                        r#"self.{field_name}.bind(site, &format!("{{var_name}}.{field_name}"));"#
                    )
                }
                None => {
                    format!(r#"self.{i}.bind(site, &format!("{{var_name}}.{i}"));"#)
                }
            };
            stmt
        })
        .collect::<Vec<_>>()
        .join("\n");

    let generic_params = generic_params_to_string(generics);
    let generic_args = generic_args_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    format!(
        r#"
        impl{generic_params} SiteBind for {struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                {bind_calls}
            }}
        }}
        impl{generic_params} SiteBind for &{struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}

/// Implements the BindToSite trait on some Enum. See comment for `create_struct_bind_impl`.
fn create_enum_bind_impl(
    enum_name: &str,
    variants: &[ast::Variant],
    generics: &ast::Generics,
) -> String {
    let arms: Vec<String> = variants
        .iter()
        .map(|variant| {
            let vname = variant.ident.as_str();
            match &variant.data {
                ast::VariantData::Unit(_) => {
                    format!("{enum_name}::{vname} => {{}}")
                }
                ast::VariantData::Tuple(fields, _) => {
                    let vars: Vec<String> = (0..fields.len()).map(|i| format!("f{i}")).collect();
                    let pattern = vars.join(", ");
                    let binds = vars
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            format!(r#"(*{v}).bind(site, &format!("{{var_name}}::{vname}.{i}"));"#)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{enum_name}::{vname}({pattern}) => {{ {binds} }}")
                }
                ast::VariantData::Struct { fields, .. } => {
                    let field_names: Vec<&str> = fields
                        .iter()
                        .filter_map(|f| Some(f.ident.as_ref()?.as_str()))
                        .collect();
                    let pattern = field_names.join(", ");
                    let binds = field_names
                        .iter()
                        .map(|name| {
                            format!(
                                r#"(*{name}).bind(site, &format!("{{var_name}}::{vname}.{name}"));"#
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{enum_name}::{vname} {{ {pattern} }} => {{ {binds} }}")
                }
            }
        })
        .collect();

    let arms_str = arms.join("\n");

    let generic_params = generic_params_to_string(generics);
    let generic_args = generic_args_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    format!(
        r#"
        impl{generic_params} SiteBind for {enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                match self {{
                    {arms_str}
                }}
            }}
        }}
        impl{generic_params} SiteBind for &{enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}
