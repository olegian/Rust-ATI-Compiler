use rustc_ast as ast;
use rustc_ast::Ty;
use rustc_span::{Ident, sym};

/// TODO: actually implement this
pub fn is_function_main(ident: &Ident) -> bool {
    ident.as_str() == "main"
}

/// Check if we should skip this function
pub fn is_function_skipped(ident: &Ident, attrs: &[ast::Attribute]) -> bool {
    // Skip main function
    if is_function_main(ident) {
        return true;
    }

    // TODO: figure out how to define untracked funcs
    // fun fact, you can pull a lot more info off of the item:
    // i.e. skip test functions.
    for attr in attrs {
        if let ast::AttrKind::Normal(normal_attr) = &attr.kind {
            let path_str = normal_attr
                .item
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.as_str())
                .collect::<Vec<_>>()
                .join("::");

            if path_str == "test" || path_str == "cfg" {
                return true;
            }
        }
    }

    false
}

pub fn is_expr_tupled(expr_kind: &ast::ExprKind) -> bool {
    !matches!(expr_kind, ast::ExprKind::Struct(_))
}

// this function is very similar to ast::TyKind::maybe_scalar
// but I'm leaving it here so that we have more control over it
pub fn is_type_tupled(ty: &Ty) -> bool {
    let ty = ty.peel_refs(); // ignore & and &mut, we care about actual type
    let Some(ty_sym) = ty.kind.is_simple_path() else {
        return false; // unit type then, which idt we need to track at all
    };

    matches!(
        ty_sym,
        sym::i8
            | sym::i16
            | sym::i32
            | sym::i64
            | sym::i128
            | sym::u8
            | sym::u16
            | sym::u32
            | sym::u64
            | sym::u128
            | sym::f16
            | sym::f32
            | sym::f64
            | sym::f128
            | sym::char
            | sym::bool
    )
}
