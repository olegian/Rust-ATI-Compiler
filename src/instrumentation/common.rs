use rustc_ast as ast;
use rustc_ast::Ty;
use rustc_span::{sym};

/// Returns true if the passed in node represents a type which 
/// is tupled at the top level (does not search through generics)
pub fn is_type_tupled(ty: &Ty) -> bool {
    if let ast::TyKind::Path(_, ast::Path { ref segments, .. }) = ty.kind {
        segments[0].ident.as_str() == "TaggedValue"
    } else {
        false
    }
}

/// Determines whether or not the passed in type can be converted into
/// a TaggedValue. Modify the below list to add/remove tupled types.
pub fn can_type_be_tupled(ty: &Ty) -> bool {
    // this function is very similar to ast::TyKind::maybe_scalar
    // but I'm leaving it here so that we have more control over it
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

/// Converts an ast Path Ty into the full type string,
// TODO: I'm actually not sure what this will do with unit types. Could just work automatically
// TODO: probably a good idea to make this return a Result in case of poorly formatted type string
pub fn expand_path_string(ty_path: &ast::Path) -> String {
    ty_path
        .segments
        .iter()
        .map(|segment| {
            let ident = segment.ident.as_str().to_string();

            if let Some(box ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                args,
                ..
            })) = &segment.args
            {
                // recursively resolve generic type args:
                // like Vec<Result<u32, ()>>
                let bracketed = args
                    .iter()
                    .map(|arg| {
                        if let ast::AngleBracketedArg::Arg(ast::GenericArg::Type(ty)) = arg {
                            if let box ast::Ty {
                                kind: ast::TyKind::Path(_, path),
                                ..
                            } = ty
                            {
                                // recursively expand those types
                                expand_path_string(path)
                            } else {
                                // this should never happen, as the types in < ... > in a
                                // path type should also be paths themselves
                                panic!();
                            }
                        } else {
                            // TODO: handle lifetimes (different GenericArg variant)?
                            todo!();
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("{ident}<{bracketed}>")
            } else {
                // no generic arguments in type string
                ident
            }
        })
        .collect::<Vec<_>>()
        .join("::")
}

/// Stores all information discovered by the UpdateFnDeclsVisitor about functions
/// that is necessary to create stub versions of all tracked functions.
#[derive(Debug)]
pub struct FnInfo {
    // FIXME: I honestly don't like the Boxes here, feels like simple
    // references will live long enough and avoid unnecessary clones
    pub params: Vec<Box<ast::Param>>,
    pub return_ty: Box<ast::FnRetTy>,
    // might want to add things like this to create full fledged stubs
    // visibility:
}

impl FnInfo {
    /// Creates string representations of the statements from ati.rs required 
    /// to bind all input parameters to the enter and exit sites.
    fn create_param_binds(&self, site_name: &str) -> String {
        self.params
            .iter()
            .filter(|param| is_type_tupled(&param.ty))
            .map(|param| {
                if let ast::PatKind::Ident(_, ref ident, _) = param.pat.kind {
                    let param_name = ident.as_str();
                    format!(
                        r#"
                        {site_name}.bind(stringify!({param_name}), {param_name});
                    "#
                    )
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Reads in self.params and constructs the string
    /// of parameter declarations to use for this function
    /// 
    /// In other words, returns the string described by <...>
    /// `fn my_foo(< a: u32, b: f64 >);`
    // FIXME: probably combined this function with create_passed_params
    fn create_param_decls(&self) -> String {
        self.params
            .iter()
            .map(|param| {
                if let ast::Param {
                    pat:
                        box ast::Pat {
                            kind: ast::PatKind::Ident(_, ref ident, _),
                            ..
                        },
                    ty:
                        box ast::Ty {
                            kind: ast::TyKind::Path(_, ref path),
                            ..
                        },
                    ..
                } = **param
                {
                    let param_name = ident.as_str();
                    let param_ty = expand_path_string(path);

                    format!(r#"{param_name}: {param_ty}"#)
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reads in self.params and constructs the string
    /// of parameters to pass into the *_unstubbed version 
    /// of the function.
    /// 
    /// In other words, returns the string described by <...>
    /// `let res = foo_unstubbed(< a, b >);``
    fn create_passed_params(&self) -> String {
        self.params
            .iter()
            .map(|param| {
                if let ast::Param {
                    pat:
                        box ast::Pat {
                            kind: ast::PatKind::Ident(_, ref ident, _),
                            ..
                        },
                    ..
                } = **param
                {
                    ident.as_str()
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reads self.return_ty and converts the node
    /// into a regular type string. Return None if
    /// the return type is ().
    fn create_return_type(&self) -> Option<String> {
        if let ast::FnRetTy::Ty(box ast::Ty {
            kind: ast::TyKind::Path(_, ref path),
            ..
        }) = *self.return_ty
        {
            Some(expand_path_string(path))
        } else {
            None
        }
    }

    /// Creates function stubs that manage ::ENTER and ::EXIT information,
    /// and properly invoke the function described by self.
    pub fn create_fn_stub(&self, name: &str) -> String {
        if name == "main" {
            // TODO: environment stuff for main
            // this is kind of a silly stub for now...
            return format!(
                r#"
                fn main() {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main::ENTER));
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    main_unstubbed();

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main::EXIT));
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    ATI_ANALYSIS.lock().unwrap().report();
                }}
            "#
            );
        }

        let enter_param_binds = self.create_param_binds("site_enter");
        let exit_param_binds = self.create_param_binds("site_exit");
        let param_decls = self.create_param_decls();
        let params_passed = self.create_passed_params();
        let ret_ty = self.create_return_type();
        // TODO: do we want to add the params to site_exit before or after the function executes?
        // as in, do we do the site_exit stuff before *_unstubbed, or after?
        if let Some(ret_ty) = ret_ty {
            // with a return value
            format!(
                r#"
                fn {name}({param_decls}) -> {ret_ty} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::ENTER));
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let res = {name}_unstubbed({params_passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::EXIT));
                    {exit_param_binds}
                    site_exit.bind(stringify!(RET), res);
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    return res;
                }}
            "#
            )
        } else {
            // without a return value
            format!(
                r#"
                fn {name}({param_decls}) {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::ENTER));
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    {name}_unstubbed({params_passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::EXIT));
                    {exit_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    ATI_ANALYSIS.lock().unwrap().report();
                }}
            "#
            )
        }
    }
}

// fun fact, you can pull a lot more info off of the item node:
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