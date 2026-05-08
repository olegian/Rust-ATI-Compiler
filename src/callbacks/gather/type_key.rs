//! Defines a namespace key for an impl-method's enclosing impl block, that encodes
//! both the self-type and optionally the name of the trait being implemented.
//!
//! Both the self-type and the trait paths are stored as fully qualified paths. This allows them
//! to be used to consistently lookup specific function information stored within the
//! `FnIndex`, inside the
//! [FirstPassInfo](crate::callbacks::gather::first_pass_info::FirstPassInfo) struct passed
//! between the first and second compilation.
//!
//! [TypeKey]s must be constructed from both AST and HIR information, which have different
//! input information available regarding the specific impl block being considered, and use
//! two different path representations. Therefore, use [TypeKey::try_from_ast] and
//! [TypeKey::try_from_hir] appropriately, before passing the result to the
//! `FnIndex`.

/// A cross-compilation stable key representing a `(self_type, of_trait?)` pair.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TypeKey {
    /// `::`-joined path of the impl's self type,
    /// Generic args are dropped (`impl Foo<u32>` and `impl<T> Foo<T>`
    /// both produce "Foo").
    pub self_path: String,
    /// `Some(path)` for `impl Trait for T`, `None` for inherent impls. Same
    /// `::`-joined ident-only format as `self_path`.
    pub trait_path: Option<String>,
}

impl std::fmt::Display for TypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.trait_path {
            Some(t) => write!(f, "{} as {}", self.self_path, t),
            None => f.write_str(&self.self_path),
        }
    }
}

impl TypeKey {
    /// Constructor for non-trait based impl blocks.
    pub fn inherent(self_path: impl Into<String>) -> Self {
        Self {
            self_path: self_path.into(),
            trait_path: None,
        }
    }

    /// Constructor for trait based impl blocks
    pub fn trait_impl(self_path: impl Into<String>, trait_path: impl Into<String>) -> Self {
        Self {
            self_path: self_path.into(),
            trait_path: Some(trait_path.into()),
        }
    }

    /// Creates a TypeKey for an impl block, derived from its `self_ty` and `of_trait`.
    ///
    /// Returns `None` when either path can't be canonicalized.
    pub fn try_from_ast(
        of_trait: Option<&rustc_ast::TraitRef>,
        self_ty: &rustc_ast::Ty,
    ) -> Option<TypeKey> {
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

    /// Creates a TypeKey for the impl block that contains `method_ldid`. Walks the impl
    /// HIR node's `self_ty` and `of_trait` paths and joins ident-only segments
    /// with `::`.
    ///
    /// Returns None when the impl's self-type isn't a resolved path
    /// (slice/array/tuple/ref/trait-object/fn-pointer self-types)
    // FIXME:  this could be more robust, we probably can support above types
    pub fn try_from_hir<'tcx>(
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
        method_ldid: rustc_span::def_id::LocalDefId,
    ) -> Option<TypeKey> {
        let impl_ldid = tcx.local_parent(method_ldid);
        let rustc_hir::Node::Item(rustc_hir::Item {
            kind:
                rustc_hir::ItemKind::Impl(rustc_hir::Impl {
                    self_ty, of_trait, ..
                }),
            ..
        }) = tcx.hir_node_by_def_id(impl_ldid)
        else {
            return None;
        };

        let self_path_str = hir_ty_canonical(self_ty)?;
        let trait_path_str = match of_trait {
            Some(header) => Some(hir_path_canonical(header.trait_ref.path)?),
            None => None,
        };

        Some(match trait_path_str {
            Some(t) => TypeKey::trait_impl(self_path_str, t),
            None => TypeKey::inherent(self_path_str),
        })
    }
}

/// Creates a canonical `::`-joined string form of an AST path.
fn ast_path_canonical(path: &rustc_ast::Path) -> Option<String> {
    let mut parts = Vec::with_capacity(path.segments.len());
    for seg in path.segments.iter() {
        parts.push(ast_segment_canonical(seg)?);
    }
    Some(parts.join("::"))
}

/// Canonicalizes a single segment of an AST path.
fn ast_segment_canonical(seg: &rustc_ast::PathSegment) -> Option<String> {
    let ident = seg.ident.name.to_string();
    let Some(args) = &seg.args else {
        return Some(ident);
    };
    let rustc_ast::GenericArgs::AngleBracketed(args) = args.as_ref() else {
        return None;
    };

    let mut rendered = Vec::new();
    for arg in args.args.iter() {
        let rustc_ast::AngleBracketedArg::Arg(generic_arg) = arg else {
            return None;
        };
        let s = match generic_arg {
            rustc_ast::GenericArg::Lifetime(lt) => lt.ident.name.to_string(),
            rustc_ast::GenericArg::Type(ty) => ast_ty_canonical(ty)?,
            rustc_ast::GenericArg::Const(_) => panic!(
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

/// Canonicalizes an AST Path type name
fn ast_ty_canonical(ty: &rustc_ast::Ty) -> Option<String> {
    if matches!(ty.kind, rustc_ast::TyKind::Infer) {
        panic!(
            "DATIR does not support inferred (`_`) generic arguments in impl-block \
             paths; see ast_ty_canonical"
        );
    }
    let rustc_ast::TyKind::Path(_, path) = &ty.kind else {
        return None;
    };
    ast_path_canonical(path)
}

/// HIR counterpart to `ast_path_canonical`. Creates a
/// `::`-joined `ident<args>` form string.
///
/// Returns None on non-`AngleBracketed` args, associated-type
/// constraints, const generic args, and non-path types as type args.
fn hir_path_canonical(path: &rustc_hir::Path<'_>) -> Option<String> {
    // FIXME: support above.
    let mut parts = Vec::with_capacity(path.segments.len());
    for seg in path.segments.iter() {
        parts.push(hir_segment_canonical(seg)?);
    }
    Some(parts.join("::"))
}

/// Constructs the canonical representation of a single HIR path segment.
fn hir_segment_canonical(seg: &rustc_hir::PathSegment<'_>) -> Option<String> {
    let ident = seg.ident.name.to_string();
    let Some(args) = seg.args else {
        return Some(ident);
    };

    if !args
        .parenthesized
        .eq(&rustc_hir::GenericArgsParentheses::No)
    {
        return None;
    }

    let mut rendered = Vec::new();
    for arg in args.args.iter() {
        let s = match arg {
            rustc_hir::GenericArg::Lifetime(lt) => lt.ident.name.to_string(),
            rustc_hir::GenericArg::Type(ty) => hir_ty_canonical(ty.as_unambig_ty())?,
            rustc_hir::GenericArg::Const(_) => panic!(
                "DATIR does not support const generic arguments in impl-block paths \
                 (encountered in segment `{}`); see hir_segment_canonical",
                seg.ident.name
            ),
            rustc_hir::GenericArg::Infer(_) => panic!(
                "DATIR does not support inferred (`_`) generic arguments in impl-block \
                 paths (encountered in segment `{}`); see hir_segment_canonical",
                seg.ident.name
            ),
        };
        rendered.push(s);
    }

    // FIXME: not really sure what to do with constraints, skipping for now.
    if !args.constraints.is_empty() {
        return None;
    }

    if rendered.is_empty() {
        Some(ident)
    } else {
        Some(format!("{ident}<{}>", rendered.join(",")))
    }
}

/// Constructs the canonical representation of a HIR type
fn hir_ty_canonical(ty: &rustc_hir::Ty<'_>) -> Option<String> {
    let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = ty.kind else {
        return None;
    };
    hir_path_canonical(path)
}
