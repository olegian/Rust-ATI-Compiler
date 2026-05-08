//! Injects the runtime library, and creates shim functions and compound type trait implementations.
//! 
//! Following transforming of the existing AST, there is some additional code that needs to be
//! generated and injected into the compiled crate. This inserted code is namely used
//! to perform ENTER/EXIT site management and provide user-defined compound types with
//! implementation of special runtime-library traits.
//!
//! Further, the runtime libary itself must be injected, alongside feature flags for a few unstable
//! rust features.
//!
//! This module contains the code to perform that, done at the end of the Instrument
//! compilation callback in [crate::callbacks::instrument].

use crate::{callbacks::gather::first_pass_info::FirstPassInfo, config::DatirConfig};

mod common;
mod data_types;
pub mod define_types;
mod function;
mod methods;

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
    krate: &mut rustc_ast::Crate,
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
    items: &mut thin_vec::ThinVec<Box<rustc_ast::Item>>,
    mod_path: &str,
    psess: &rustc_session::parse::ParseSess,
) {
    // inner fns, generated inherent impl blocks holding inner methods,
    // and SiteBind impls for user-defined struct/enum types.
    let mut new_items: Vec<Box<rustc_ast::Item>> = Vec::new();

    for item in items.iter_mut() {
        match &mut item.kind {
            // Free functions
            rustc_ast::ItemKind::Fn(func) => {
                function::generate_function_shim(
                    datir_config,
                    first_pass,
                    psess,
                    mod_path,
                    func,
                    &mut new_items,
                );
            }

            // Struct definition, emit BindToSite impl.
            rustc_ast::ItemKind::Struct(
                ident,
                generics,
                rustc_ast::VariantData::Struct { fields, .. },
            )
            | rustc_ast::ItemKind::Struct(
                ident,
                generics,
                rustc_ast::VariantData::Tuple(fields, ..),
            ) => {
                data_types::generate_struct_impls(
                    psess,
                    ident.as_str(),
                    fields,
                    generics,
                    &mut new_items,
                );
            }

            // Enum definition, emit BindToSite impl.
            rustc_ast::ItemKind::Enum(ident, generics, rustc_ast::EnumDef { variants }) => {
                data_types::generate_enum_impls(
                    psess,
                    ident.as_str(),
                    variants,
                    generics,
                    &mut new_items,
                );
            }

            // Impl block (inherent or trait): replace each method body with a
            // stub and emit a sibling inherent impl holding the inner methods.
            rustc_ast::ItemKind::Impl(impl_block) => {
                methods::generate_method_shims(
                    datir_config,
                    first_pass,
                    psess,
                    mod_path,
                    impl_block,
                    &mut new_items,
                );
            }

            // Recurse into submodules, update mod path.
            rustc_ast::ItemKind::Mod(_, mod_ident, rustc_ast::ModKind::Loaded(sub_items, _, _)) => {
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

    // actually inject all collected items into the krate
    for new_item in new_items {
        items.push(new_item);
    }

    if !mod_path.is_empty() {
        // if we are in a submodule / non-root file, then import root
        // to make types available.
        let imports = crate::callbacks::parsing::parse_items(psess, "use crate::*;".into(), None);
        for import in imports {
            items.insert(0, import);
        }
    }
}
