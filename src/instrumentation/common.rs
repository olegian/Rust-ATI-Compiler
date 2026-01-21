use rustc_ast as ast;
use rustc_ast::Ty;
use rustc_span::{Ident, sym};

pub fn is_expr_tupled(expr_kind: &ast::ExprKind) -> bool {
    !matches!(expr_kind, ast::ExprKind::Struct(_))
}

pub fn is_type_tupled(ty: &Ty) -> bool {
    if let ast::TyKind::Path(_, ast::Path { ref segments, .. }) = ty.kind {
        segments[0].ident.as_str() == "TaggedValue"
    } else {
        false
    }
}

/// Determines whether or not the passed in type can be converted into
/// a TaggedValue
// this function is very similar to ast::TyKind::maybe_scalar
// but I'm leaving it here so that we have more control over it
pub fn can_type_be_tupled(ty: &Ty) -> bool {
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

#[derive(Debug, Clone, Copy)]
pub enum AtiFnType {
    Main,
    Tracked,
    Untracked,
}

/// Returns whether the passed in functions are tracked or not.
pub fn function_type(ident: &Ident, attrs: &[ast::Attribute]) -> AtiFnType {
    // leaving this here to let us easily map more specific stuff later
    // "" => AtiFnType::Tracked
    // _ => AtiFnType::Untracked
    match ident.as_str() {
        "main" => AtiFnType::Main,
        _ => AtiFnType::Tracked,
    }
}

// TODO: figure out how to define untracked funcs
// fun fact, you can pull a lot more info off of the item:
// i.e. skip test functions.
// for attr in attrs {
//     if let ast::AttrKind::Normal(normal_attr) = &attr.kind {
//         let path_str = normal_attr
//             .item
//             .path
//             .segments
//             .iter()
//             .map(|seg| seg.ident.as_str())
//             .collect::<Vec<_>>()
//             .join("::");

//         if path_str == "test" || path_str == "cfg" {
//             return true;
//         }
//     }
// }
