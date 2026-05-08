//! Generates a shim method for a single method within an impl block.
//!
//! See [crate::callbacks::codegen::methods] for more information on method shims.
//!
//! This file is analogous to [crate::callbacks::codegen::function], but handles the method-specific edge
//! cases and caveats described in [crate::callbacks::codegen::methods], namely Self-qualification, and
//! managing the `self` parameter.

use crate::{
    callbacks::codegen::common::{
        build_inner_call_args, create_param_binds, generic_params_to_string, get_param_name,
        get_unique_inner_name, is_dead, where_clause_to_string,
    },
    callbacks::codegen::methods::self_qualifier::SelfPathQualifier,
    callbacks::gather::first_pass_info::{FirstPassInfo, FnNamespace},
    callbacks::gather::type_key::TypeKey,
    callbacks::parsing,
    config::DatirConfig,
};

/// For a single method, generates the appropriate shim.
/// 
/// Replaces a single impl-item's method body with a stub that opens
/// ENTER/EXIT sites and calls a freshly-named inner method, and pushes
/// the corresponding inner-method template + the original body onto
/// the caller's accumulators. Non-fn associated items are skipped.
///
/// The trait_segs argument carries the trait reference's path segments
/// for trait impls; if Some, every `Self::X` path within the method's
/// signature and body is rewritten to its `<Self as Trait>::X` form so
/// the generated inherent inner-impl can resolve associated items.
pub fn generate_method_shim(
    datir_config: &DatirConfig,
    first_pass: &FirstPassInfo,
    psess: &rustc_session::parse::ParseSess,
    mod_path: &str,
    type_key: &TypeKey,
    trait_segs: Option<&[rustc_ast::PathSegment]>,
    known_names: &std::collections::HashSet<String>,
    assoc_item: &mut rustc_ast::AssocItem,
    inner_templates: &mut Vec<String>,
    taken_bodies: &mut Vec<Box<rustc_ast::Block>>,
) {
    // we only care about functions in impl blocks.
    let rustc_ast::AssocItemKind::Fn(box rustc_ast::Fn {
        ident,
        generics: method_generics,
        sig: rustc_ast::FnSig { decl, .. },
        body,
        ..
    }) = &mut assoc_item.kind
    else {
        return;
    };

    // same strategy here as free functions.
    let orig_name = ident.as_str().to_string();
    let inner_name = get_unique_inner_name(&orig_name, known_names);
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
        .fns
        .lookup(mod_path, FnNamespace::Method(type_key), ident.as_str())
        .unwrap_or_else(|| {
            panic!(
                "stub generation could not find a FnBasePptName for method \
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
    if let Some(segs) = trait_segs {
        let mut qualifier = SelfPathQualifier { trait_segs: segs };

        for param in decl.inputs.iter_mut() {
            rustc_ast::mut_visit::MutVisitor::visit_ty(&mut qualifier, &mut param.ty);
        }
        if let rustc_ast::FnRetTy::Ty(ret_ty) = &mut decl.output {
            rustc_ast::mut_visit::MutVisitor::visit_ty(&mut qualifier, ret_ty);
        }

        rustc_ast::mut_visit::MutVisitor::visit_block(&mut qualifier, &mut orig_body);
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
        .enter_ppt(entry)
        .expect("ENTER ppt missing, should have been validated in pass 1");
    let exit_ppt = datir_config
        .decls_file
        .exit_ppt(entry)
        .expect("EXIT ppt missing, should have been validated in pass 1");

    // replace the existing method body with the stub code.
    let wrapper_src = build_method_wrapper_block(
        entry,
        &inner_name,
        &decl.inputs,
        &decl.output,
        enter_ppt,
        exit_ppt,
    );
    let parsed_wrapper = parsing::parse_expr(psess, wrapper_src);
    let rustc_ast::ExprKind::Block(new_block, _) = parsed_wrapper.kind else {
        panic!(
            "wrapper-block source for method `{type_key}::{orig_name}` \
             did not parse as a block"
        );
    };
    *body = Some(new_block);
}

/// Source for an inner method signature with an empty placeholder body.
/// 
/// The caller parses this template, then transplants the user's original body.
fn build_inner_method_template(
    inner_name: &str,
    method_generics: &rustc_ast::Generics,
    inputs: &[rustc_ast::Param],
    output: &rustc_ast::FnRetTy,
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
    let other = iter
        .map(|p| {
            format!(
                "mut {}: {}",
                get_param_name(p),
                rustc_ast_pretty::pprust::ty_to_string(&p.ty)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let declared = match (receiver_str.is_empty(), other.is_empty()) {
        (true, _) => other,
        (false, true) => receiver_str.to_string(),
        (false, false) => format!("{receiver_str}, {other}"),
    };
    let ret = match output {
        rustc_ast::FnRetTy::Ty(t) => format!(" -> {}", rustc_ast_pretty::pprust::ty_to_string(t)),
        rustc_ast::FnRetTy::Default(_) => String::new(),
    };
    format!("fn {inner_name}{generic_params}({declared}){ret}{where_clause} {{ }}")
}

/// Constructs the body for a method's shim. 
/// 
/// A small difference from the free-fn version: the self parameter needs to
/// be correctly forwarded when invoking the inner method. The receiver is
/// bound under the name `"self"`, and is filtered by liveness in the same way
/// as other formals (an owned non-Copy `self` is dead at exit and gets
/// skipped).
fn build_method_wrapper_block(
    base_ppt_name: &str,
    inner_name: &str,
    inputs: &[rustc_ast::Param],
    output: &rustc_ast::FnRetTy,
    enter_ppt: &decls_gen::ProgramPoint,
    exit_ppt: &decls_gen::ProgramPoint,
) -> String {
    let receiver = determine_receiver_kind(inputs);

    let mut inputs_iter = inputs.iter();
    if !matches!(receiver, ReceiverKind::None) {
        inputs_iter.next();
    }
    let non_self: Vec<&rustc_ast::Param> = inputs_iter.collect();

    let passed = build_inner_call_args(non_self.iter().copied());

    let call_expr = match receiver {
        ReceiverKind::None => format!("Self::{inner_name}({passed})"),
        _ => format!("self.{inner_name}({passed})"),
    };

    let self_bind = |site_name: &str, ppt: &decls_gen::ProgramPoint| -> String {
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

    let return_var_name = decls_gen::decls::RETURN_VAR_NAME;

    match output {
        rustc_ast::FnRetTy::Ty(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let res = {call_expr};

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                res.bind(&mut site_exit, "{return_var_name}");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                return res;
            }}"#
        ),
        rustc_ast::FnRetTy::Default(_) => format!(
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

/// Describes how a method receives its `self` argument
#[derive(Debug)]
enum ReceiverKind {
    /// Associated function with no self parameter.
    None,
    /// Takes self
    Value,
    /// Takes &self
    Ref,
    /// Takes &mut self
    RefMut,
}

/// Constructs a [`ReceiverKind`], based off the input parameter list.
///  
/// Determines whether a list of parameters being passed to some
/// function or method accepts self, &self, &mut self.
fn determine_receiver_kind(params: &[rustc_ast::Param]) -> ReceiverKind {
    let Some(first) = params.first() else {
        return ReceiverKind::None;
    };

    match &first.ty.kind {
        rustc_ast::TyKind::ImplicitSelf => ReceiverKind::Value,
        rustc_ast::TyKind::Ref(_, rustc_ast::MutTy { mutbl, .. }) => {
            if matches!(mutbl, rustc_ast::Mutability::Mut) {
                ReceiverKind::RefMut
            } else {
                ReceiverKind::Ref
            }
        }
        _ => ReceiverKind::None,
    }
}
