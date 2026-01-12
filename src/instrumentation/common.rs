use rustc_ast as ast;
use rustc_span::Ident;

/// TODO: actually implement this
pub fn is_function_main(ident: &Ident) -> bool {
    true
}

/// Check if we should skip this function
pub fn is_function_skipped(ident: &Ident, attrs: &[ast::Attribute]) -> bool {
    // Skip main function
    if ident.as_str() == "main" {
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
