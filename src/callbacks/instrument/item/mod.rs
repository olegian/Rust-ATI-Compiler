//! Defines [`transform_item`], which governs how rustc_ast::Items are transformed to include
//! instrumentation.
//!
//! Items are top level nodes within the compiled crate, governing function definitions, struct
//! definitions, enum definitions, trait definitions and sub modules.
//!
//! Items consist of other AST nodes that require expression and type-level transformation.
//! Expressions are transformed according to [crate::callbacks::instrument::expr], types are transformed
//! according to [crate::callbacks::instrument::types].
//!
//! Functions have their bodies instrumented, and input parameter and return types transformed.
//! Struct definitions have thier field types transformed.
//! Enum definitions have the fields of each variant transformed.
//! Trait definitions have default function implementations instrumented.
//! Impl blocks have each method instrumented, and input parameter and return types transformed
//! Items within submodules are recursively transformed.

use crate::callbacks::instrument::instrument_visitor::InstrumentingVisitor;

mod bodies;
pub mod data_types;

/// Recursively instruments this item, with respect to the item kind.
pub fn transform_item<'session>(
    visitor: &mut InstrumentingVisitor<'session>,
    item: &mut rustc_ast::Item,
) {
    match &mut item.kind {
        rustc_ast::ItemKind::Fn(..) => {
            bodies::transform_fn(visitor, item);
        }
        rustc_ast::ItemKind::Impl(..) => {
            bodies::transform_impl(visitor, item);
        },
        rustc_ast::ItemKind::Trait(..) => {
            bodies::transform_trait(visitor, item);
        },
        rustc_ast::ItemKind::Struct(..) => {
            data_types::transform_struct(visitor, item);
        },
        rustc_ast::ItemKind::Enum(..) => {
            data_types::transform_enum(visitor, item);
        },

        // recurse into submodules, updating the visitors
        // active mod path.
        rustc_ast::ItemKind::Mod(_safety, ident, rustc_ast::ModKind::Loaded(sub_items, _, _)) => {
            // we expect to use visitor.mod_path 
            // to perform lookups in FirstPassInfo quite often,
            // but recurse into submodules quite rarely. For that reason
            // we transform the mod path in place.
            let saved_len = visitor.mod_path.len();
            if saved_len > 0 {
                visitor.mod_path.push_str("::");
            }
            visitor.mod_path.push_str(ident.as_str());

            // instrument each item
            for sub_item in sub_items.iter_mut() {
                transform_item(visitor, sub_item);
            }

            visitor.mod_path.truncate(saved_len);
        }

        // all other no-op items
        rustc_ast::ItemKind::ExternCrate(..)
        | rustc_ast::ItemKind::Use(..)
        | rustc_ast::ItemKind::Static(..)
        | rustc_ast::ItemKind::Const(..)
        | rustc_ast::ItemKind::ConstBlock(..)
        | rustc_ast::ItemKind::ForeignMod(..)
        | rustc_ast::ItemKind::GlobalAsm(..)
        | rustc_ast::ItemKind::TyAlias(..)
        | rustc_ast::ItemKind::Union(..)
        | rustc_ast::ItemKind::TraitAlias(..)
        | rustc_ast::ItemKind::MacCall(..)
        | rustc_ast::ItemKind::MacroDef(..)
        | rustc_ast::ItemKind::Delegation(..)
        | rustc_ast::ItemKind::Mod(..)  // unloaded modules
        | rustc_ast::ItemKind::DelegationMac(..) => {}
    }
}
