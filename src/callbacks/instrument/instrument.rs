//! Defines a visitor which modifies the AST to include the necessary expression and type-level
//! changes to instrument the compiled target.
//!
//! Abstracting away a lot of details, a rust crate consists of a list of Items. Abusing notation
//! a little bit, and omitting a few variants/fields, these items break down roughly into these
//! definitions:
//! ```
//! Item      := FnDef | StructDef | EnumDef | ImplBlock | SubModule
//! FnDef     := { Name, FnBody, FnSig }
//! StructDef := { Name, [ Field ] }
//! EnumDef   := { Name, [ Variant ] }
//! ImplBlock := { SelfTy, OfTrait?, [ Item::FnDef ] }
//! SubModule := [ Item ]
//!
//! FnBody  := [ Stmt ]
//! FnSig   := { [ Field ], Ty? }   (list of inputs and optionally a return type)
//! Field   := { Name, Ty }
//! Variant := { Name, [ Field ] }
//! ...
//! ```
//!
//! [`InstrumentingVisitor`] is going to traverse the entire AST, turning all bodies that will
//! execute at runtime into instrumented versions.
//!
//! Statements which themselves consist of expressions, will be rewritten to preserve the behavior
//! of the original program, additionally now keeping track of value interactions. This is done
//! by "tupling" each atomic value (i.e. `u32 --> Tagged<u32> == (Id, u32)`) with an Id, which will
//! uniquely identify that specific value. Then, whenever this tagged value interacts with another
//! (usually in a Binary expression, but see `./expr/ops.rs` for specifics), the corresponding Ids
//! will be merged within a Union-Find structure, which is stored within an injected `ATI_ANALYSIS`
//! global. See [crate::ati::ati] for a description of how the global interaction state is managed.
//!
//! Arrays, slices, and ranges are all composed of "inner" types, yet the collection itself also
//! receives a separate tag for the length. This means we represent arrays/slices as:
//!  - `[T; N]` --> `Tagged<[Tagged<T>; N]>`
//!  - `[T]`    --> `Tagged<[Tagged<T>]>`
//!
//! However, slices cannot be constructed without being behind a pointer type, and therefore
//! - `&[T]`  --> `&Tagged<[Tagged<T>]>` --> `TaggedRef<[Tagged<T>]>`.
//! This bring us to the next point, importantly, DATIR utilizes a different representation of
//! references than what one might expect.
//!
//! Consider the following:
//! ```rust
//! let v1 = 10;
//! let v2 = 20;
//!
//! let shared_ref = &v1;
//! let mut_ref = &mut v2;
//! ```
//!
//! If we were to naively transform the above by tupling all values with different Ids we might
//! expect something like:
//! ```rust
//! let v1 = Tagged(0, 10);
//! let v2 = Tagged(1, 20);
//!
//! let shared_ref = &v1;
//! let mut_ref = &mut v2;
//! ```
//!
//! The type of `shared_ref` is `&Tagged<i32> = &(Id, T)`, and `mut_ref` is
//! `&mut Tagged<i32> = &mut (Id, T)`. This simple example poses no challenges. However, this
//! approach falls short because of memory layout requirements, both of these structures would
//! require `Id` and the referred-to `T` to be contiguous in memory.
//!
//! Consider the following indexing example:
//! ```rust
//! let array = [10; 5];
//! let slice = &array[2..];
//! ```
//!
//! Naively instrumenting, we get:
//! ```rust
//! let array = Tagged(0, [Tagged(1, 10); 5]);
//! let slice = array[(Tagged(2, 2).1)..];  ????
//! ```
//!
//! `slice` should be a reference with the same length-id as used by `array`, but it will hold a
//! completely different set of elements inside of the slice itself! We cannot guarantee that
//! the length-id and the data itself will be contiguous. Therefore, we convert `&Tagged<T>` into
//! a `TaggedRef<T> = (&Id, &T)` or a `TaggedRefMut<T> = (&mut Id, &mut T)`, by utilizing the
//! runtime library `.as_tagged_ref()` / `.as_tagged_ref_mut()` functions defined on all
//! `Tagged<T>` types. It's for this reason the first gather pass has to locate all places where a
//! reference to a Tagged type is constructed, so that the appropriate method call can be inserted.
//!
//! As a further complication of the reference model, following ownership semantics, a `&mut T`
//! does not hold ownership of the underlying `T`. To disallow aliasing, it is explicitly made
//! non-`Copy`. It is not an "owned type". However, `TaggedRefMut<T> = (&mut Id, &mut T)` is just
//! a tuple, and
//! therefore an owned type! To allow for a `TaggedRefMut<T>` to utilize the same move semantics
//! as a `&mut T`, we allow it to be explicitly "reborrowed", something that the rust compiler
//! usually automatically handles for `&mut T`s. This requires the `TaggedRefMut<T>` to be
//! dereferenced, and a second `TaggedRefMut<T>` to be constructed out of the dereferenced values.
//! The contained `&mut`s will then be of a shorter lifetime than the original `&mut`, and the new
//! `TaggedRefMut<T>` can be moved into functions freely. This is handled by the runtime library's
//! `.reborrow()` function, and explains why the first pass looks for expressions which evaluate to
//! a `&mut T`.
//!
//! Types, which appear above in things like `ImplBlock`s, but also in `Stmt`s, `FnSig`s,
//! `StructDef`s, `EnumDef`s, and other AST nodes, are appropriately rewritten to convert each
//! atomic `T` into a `Tagged<T>` version (or `&T` to a `TaggedRef<T>`, etc...).
//!
//! Items in submodules are recursively instrumented.
//!
//! Finally, we will be inserting many chained method invocations, which sometimes causes rust to
//! create a temporary rather than let-binding a variable. This results in values being dropped
//! earlier than necessary. The simplest example, instrumeting:
//! ```rust
//! let x = &(10 + 20);
//! ```
//! will result in:
//! ```rust
//! let x = (Tagged(0, 10) + Tagged(1, 20)).as_tagged_ref();
//! ```
//! The resulting `TaggedRef` will refer to the result of addition, but the result is not bound to
//! any variable! This means the result will be dropped, and the reference left dangling. The
//! hoisting mechanism (implemented in the below `flat_map_stmt` function) will further transform
//! the above into:
//! ```rust
//! let __ati_hoist0 = Tagged(0, 10) + Tagged(1, 20);
//! let x = __ati_hoist0.as_tagged_ref();
//! ```
//! to make sure the result is bound to a variable before a reference to it is also stored. This
//! mechanism is required for all added method invocations.
//!
//! There are many more finer details to making sure instrumentation is performed correctly, this
//! is only a short high-level overview. For more specific information, view the following files:
//! 1. [super::types] for how individual types are transformed into Tagged versions.
//! 2. [super::expr] for how individual expressions are transformed.
//! 3. [super::item] for how individual items are transformed.
//! 4. [super::hoisting] for how the hoisting mechanism works.
//! 5. [crate::ati::ati] for how global analysis state is managed.
//! 6. [crate::ati::arrays] for how arrays and slices are indexed with usizes and ranges.
//! 7. [crate::ati::tagged_ops] for how interactions between values are observed and recorded.
//! 8. [crate::ati::tagged] for `Tagged` / `TaggedRef` / `TaggedRefMut` definitions and further
//!    formalization.
//! 9. [crate::ati::iterators] for how tagged arrays and slices are used in for loops.

use crate::{
    callbacks::gather::first_pass_info::FirstPassInfo,
    callbacks::instrument::{expr, hoisting, item, types},
    config::DatirConfig,
};

/// Visitor which performs the AST transformation to insert instrumentation into the
/// compiled crate.
pub struct InstrumentingVisitor<'a> {
    pub datir_config: &'a DatirConfig,
    pub first_pass: &'a FirstPassInfo,
    pub psess: &'a rustc_session::parse::ParseSess,
    pub mod_path: String,
}

impl<'a> InstrumentingVisitor<'a> {
    /// Constructor.
    pub fn new(
        psess: &'a rustc_session::parse::ParseSess,
        datir_config: &'a DatirConfig,
        first_pass: &'a FirstPassInfo,
        mod_path: impl Into<String>,
    ) -> Self {
        Self {
            datir_config,
            first_pass,
            psess,
            mod_path: mod_path.into(),
        }
    }
}

impl<'a> rustc_ast::mut_visit::MutVisitor for InstrumentingVisitor<'a> {
    /// Defining this stops the visitor from visiting rustc_ast::Params,
    /// which are contained within function signatures. These will be updated
    /// via the function item instead.
    fn visit_param(&mut self, _node: &mut rustc_ast::Param) {}

    /// Defining this stops the visitor from changing any compile time constants,
    /// like lengths of arrays.
    fn visit_anon_const(&mut self, _node: &mut rustc_ast::AnonConst) {}

    /// Transform `let x: ty` statements, into `let x: Tag(ty)`.
    fn visit_local(&mut self, local: &mut rustc_ast::Local) {
        if let Some(ty) = &mut local.ty {
            types::recursively_transform_ast_type(ty);
        }

        rustc_ast::mut_visit::walk_local(self, local);
    }

    /// Transform all visited expressions.
    fn visit_expr(&mut self, expr: &mut rustc_ast::Expr) {
        expr::transform_expr(self, expr);
    }

    /// Transform all visited module items.
    fn visit_item(&mut self, item: &mut rustc_ast::Item) {
        item::transform_item(self, item)
    }

    /// After transforming all expressions, iterate through all statements and
    /// hoist any necessary method calls.
    /// The hoisting transformation happens after the statement is walked and transformed.
    fn flat_map_stmt(&mut self, stmt: rustc_ast::Stmt) -> smallvec::SmallVec<[rustc_ast::Stmt; 1]> {
        let mut stmts = rustc_ast::mut_visit::walk_flat_map_stmt(self, stmt);
        if stmts.len() != 1 {
            return stmts;
        }

        let stmt = stmts.pop().unwrap();
        hoisting::maybe_hoist_binding(self, stmt)
    }
}
