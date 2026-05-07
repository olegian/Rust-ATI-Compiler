//! Constructs shim functions for all implemented methods of user-defined compound types.
//!
//! The overall structure of each shim (and the overall transformation process) is analogous to
//! what occurs for free functions. For details on shim functions, see [crate::callbacks::codegen::function].
//!
//! When it comes to generating shims for methods, there are a few more caveats however:
//! - impl blocks could have where clauses, these where clauses must be placed on the generated
//!   impl block that houses the "inner" function that contains the original logic.
//! - The name of the original function *cannot* change. If the impl block is implementing a trait,
//!   then the function name inside the trait implementation must be identical to the original
//!   function name. It's for this reason that we leave the original function where it is, and swap
//!   out the body of the function for the shim, rather than renaming the function and generating a
//!   new shim with the original name.
//! - Traits can have associated types, which are identified via the `Self::` qualifier. These
//!   types will not be available within the inner function's impl block, unless each usage
//!   of each type is "fully qualified" (in other words, if before we had a function within some
//!   trait `MyTrait` that returned `Self::SomeType`, then the inner function must rewrite the
//!   return value to be `<Self as MyTrait>::SomeType`. This rewrite is done via the visitor
//!   within [self_qualifier].

use crate::{
    callbacks::codegen::common::{generic_params_to_string, where_clause_to_string},
    callbacks::gather::first_pass_info::{FirstPassInfo, FnNamespace},
    callbacks::gather::type_key::TypeKey,
    callbacks::parsing,
    config::DatirConfig,
};

mod method;
mod self_qualifier;

/// Walks the methods within an impl block, replacing each method body with a
/// stub that opens ENTER/EXIT sites and calls a freshly-named inner method.
/// All inner methods are gathered into a single new impl block (carrying the
/// original block's generics / where clause) appended to the surrounding mod
/// via `new_items`.
///
/// Both inherent and trait impls are handled here: trait impls additionally
/// rewrite `Self::X` paths within signatures and bodies into the fully
/// qualified `<Self as Trait>::X` form so the new inner impl, which is
/// inherent, can resolve the associated items.
pub fn generate_method_shims(
    datir_config: &DatirConfig,
    first_pass: &FirstPassInfo,
    psess: &rustc_session::parse::ParseSess,
    mod_path: &str,
    impl_block: &mut rustc_ast::Impl,
    new_items: &mut Vec<Box<rustc_ast::Item>>,
) {
    let rustc_ast::Impl {
        generics: impl_generics,
        of_trait,
        self_ty,
        items: impl_items,
        ..
    } = impl_block;

    let type_name = rustc_ast_pretty::pprust::ty_to_string(self_ty);
    let type_key = TypeKey::try_from_ast(of_trait.as_deref().map(|h| &h.trait_ref), self_ty)
        .unwrap_or_else(|| {
            panic!(
                "stub generation could not derive TypeKey from impl self-type \
                 `{type_name}` in module `{mod_path}`; only path self/trait \
                 types are supported"
            )
        });
    let impl_generic_params = generic_params_to_string(impl_generics);
    let impl_where_clause = where_clause_to_string(impl_generics);

    // traits can have associated types! if any are defined and used within the
    // original body, we need to change them to be fully qualified
    // (e.g. <Self as Add>::Output). Given that we need to do this for all
    // associated functions, extract the list of segments in the trait early.
    let trait_segs: Option<thin_vec::ThinVec<rustc_ast::PathSegment>> = of_trait
        .as_deref()
        .map(|h| h.trait_ref.path.segments.clone());

    // all items in this impl will be in this namespace
    let known_names = first_pass
        .fns
        .names_in(mod_path, FnNamespace::Method(&type_key));

    let mut inner_templates: Vec<String> = Vec::new();
    let mut taken_bodies: Vec<Box<rustc_ast::Block>> = Vec::new();

    // Generate each method shim
    for assoc_item in impl_items.iter_mut() {
        method::generate_method_shim(
            datir_config,
            first_pass,
            psess,
            mod_path,
            &type_key,
            trait_segs.as_deref(),
            &known_names,
            assoc_item,
            &mut inner_templates,
            &mut taken_bodies,
        );
    }

    // we have nothing to add, so exit early.
    if inner_templates.is_empty() {
        return;
    }

    // we actually do have something to add.
    // unify every method defined in the current impl block into another impl
    // block with the same generics / where clause.
    let impl_template = format!(
        "impl{impl_generic_params} {type_name}{impl_where_clause} {{\n{}\n}}",
        inner_templates.join("\n\n"),
    );

    let mut parsed_items = parsing::parse_items(psess, impl_template, None);
    let mut impl_item = parsed_items
        .pop()
        .expect("inner-impl template did not parse into an item");
    let rustc_ast::ItemKind::Impl(rustc_ast::Impl {
        items: ref mut parsed_assoc,
        ..
    }) = impl_item.kind
    else {
        panic!("inner-impl template did not yield ItemKind::Impl");
    };

    // sanity check
    if parsed_assoc.len() != taken_bodies.len() {
        panic!(
            "inner-impl assoc count ({}) != taken body count ({}) for \
             `{type_key}` in `{mod_path}`",
            parsed_assoc.len(),
            taken_bodies.len(),
        );
    }

    for (assoc, orig_body) in parsed_assoc.iter_mut().zip(taken_bodies) {
        let rustc_ast::AssocItemKind::Fn(box rustc_ast::Fn {
            body: ref mut inner_body,
            ..
        }) = assoc.kind
        else {
            panic!("parsed inner method was not a Fn");
        };
        *inner_body = Some(orig_body);
    }

    new_items.push(impl_item);
}
