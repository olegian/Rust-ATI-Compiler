/* Provides helper functions that are used throughout this entire project.
 * Namely, this includes determining the set of types that are considered
 * able to be tagged, as well as moving between different representations of types.
*/
use rustc_ast::token::{Lit, LitKind};
use rustc_ast::{self as ast};
use rustc_middle as mir;
use rustc_span::sym;

/// Determines whether a type is a tracked primitive that can be wrapped in `Tagged<T>`.
/// Defines as a trait so that it can be shared between both MIR types and AST types
pub trait CanBeTupled {
    fn can_be_tupled(&self) -> bool;
}

impl CanBeTupled for ast::Ty {
    fn can_be_tupled(&self) -> bool {
        let ty = self.peel_refs();
        let Some(ty_sym) = ty.kind.is_simple_path() else {
            return false;
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
                | sym::isize
                | sym::usize
                | sym::bool
                | sym::char
        )
    }
}

impl CanBeTupled for mir::ty::Ty<'_> {
    fn can_be_tupled(&self) -> bool {
        self.is_integral() || self.is_floating_point() || self.is_bool() || self.is_char()
    }
}

/// Determines whether or not the passed in literal can be converted
/// into a TaggedValue. Modify the below list to enable/disable tupling literals.
pub fn can_literal_be_tupled(lit: &Lit) -> bool {
    match lit.kind {
        LitKind::Integer | LitKind::Float | LitKind::Bool | LitKind::Char => true,
        _ => false,
    }
}

// FIXME: this is equiv to Ty::peel_refs but just mutable rather than shared borrows
// there has to be a better way!
pub fn peel_refs(ty: &mut ast::Ty) -> &mut ast::Ty {
    let mut final_ty = ty;
    while let ast::TyKind::Ref(_, ast::MutTy { ref mut ty, .. })
    | ast::TyKind::Ptr(ast::MutTy { ref mut ty, .. }) = final_ty.kind
    {
        final_ty = &mut **ty;
    }

    final_ty
}

/// Takes an ast lifetime and turns it into a regular "'name" string.
fn get_lifetime_string(lifetime: &ast::Lifetime) -> String {
    lifetime.ident.to_string()
}

fn get_anon_const_string(anon_const: &ast::AnonConst) -> String {
    let ast::AnonConst {
        value:
            box ast::Expr {
                kind: ast::ExprKind::Lit(ast::token::Lit { symbol, .. }),
                ..
            },
        ..
    } = anon_const
    else {
        unreachable!("Attmempted to parse a non-Literal expression from an AnonConst");
    };

    format!("{}", symbol.as_str())
}

/// Converts an ast Ty into the full type string, recursively.
// NIT: i hate the way that I'm parsing strings here, feels like a lot of unnecessary format!s
// I also think there might be a way to go from Span -> underlying text repr. would be really nice here
pub fn get_type_string(ty_path: &ast::Ty) -> String {
    match &ty_path.kind {
        rustc_ast::TyKind::Slice(box ty) => format!("[{}]", get_type_string(ty)),
        rustc_ast::TyKind::Ref(lifetime, ast::MutTy {
            box ty,
            mutbl,
        }) => {
            let mut_str = mutbl.prefix_str();
            let lt_str = match lifetime {
                Some(lifetime) => format!("{} ", get_lifetime_string(lifetime)),
                None => "".to_string(),
            };
            let refed_type_str = get_type_string(ty);

            format!("&{lt_str}{mut_str}{refed_type_str}")
        },
        rustc_ast::TyKind::Tup(v) => {
            let types = v.iter().map(|box ty| {
                get_type_string(ty)
            }).collect::<Vec<_>>().join(", ");

            format!("({types})")
        },

        // idk what qself really does... ignoring for now
        rustc_ast::TyKind::Path(qself, path) => {
            path.segments.iter().map(|segment| {
                let ident_str = segment.ident.to_string();

                // these are the <Generic, Args> passed in to this segment
                let generics_str = if let Some(box generics) = &segment.args {
                    match generics {
                        ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs{
                            args,
                            ..
                        }) => {
                            // <'a, A, B, C>
                            let arg_list_string = args.iter().map(|arg| {
                                match arg {
                                    rustc_ast::AngleBracketedArg::Arg(generic_arg) => {
                                        match generic_arg {
                                            rustc_ast::GenericArg::Lifetime(lifetime) => get_lifetime_string(lifetime),
                                            rustc_ast::GenericArg::Type(box ty) => get_type_string(&ty),
                                            rustc_ast::GenericArg::Const(anon_const) => get_anon_const_string(anon_const),
                                        }
                                    },
                                    rustc_ast::AngleBracketedArg::Constraint(assoc_item_constraint) => {
                                        // ": Trait"
                                        // this also has to be done at some point
                                        unimplemented!();

                                    },
                                }
                            }).collect::<Vec<_>>().join(", ");

                            format!("<{arg_list_string}>")
                        },
                        ast::GenericArgs::Parenthesized(ast::ParenthesizedArgs {
                            inputs,
                            output,
                            ..
                        }) => {
                            // (A, B) -> C
                            let input_list_str = inputs.iter().map(|box input_ty| {
                                get_type_string(input_ty)
                            }).collect::<Vec<_>>().join(", ");

                            let output_str = match output {
                                rustc_ast::FnRetTy::Default(_) => "".into(),  // unit type return
                                rustc_ast::FnRetTy::Ty(box ty) => format!(" -> {}", get_type_string(ty)),
                            };

                            format!("({input_list_str}){output_str}")
                        },
                        ast::GenericArgs::ParenthesizedElided(span) => {
                            // (..)
                            // i've never even seen this before
                            unimplemented!()
                        },
                    }
                } else {
                    "".into()
                };

                format!("{ident_str}{generics_str}")
            }).collect::<Vec<_>>().join("::")
        },

        rustc_ast::TyKind::Array(box ty, ast::AnonConst {
            value: box ast::Expr {
                kind: ast::ExprKind::Lit(ast::token::Lit {
                    symbol,
                    ..
                }),
                ..
            },
            ..
        }) => {
            let inner = get_type_string(ty);
            let constant = symbol.as_str();

            let res = format!("[{inner}; {constant}]");
            // panic!("{res:?}");
            res
        },

        rustc_ast::TyKind::Array(box ty, ast::AnonConst {
            value,
            ..
        }) => {
            // panic!("Found array with non-literal size:\n{ty:#?}\n{value:#?}");
            "[ARRAY]".into()
        },

        // this should be impossible, for now error out
        rustc_ast::TyKind::ImplicitSelf |  // def necessary at some point
        rustc_ast::TyKind::MacCall(_) |
        rustc_ast::TyKind::CVarArgs |
        rustc_ast::TyKind::Pat(_, _) |
        rustc_ast::TyKind::Err(_) |
        rustc_ast::TyKind::Dummy |
        rustc_ast::TyKind::Paren(_) |
        rustc_ast::TyKind::TraitObject(_, _) | // prob necessary at some point
        rustc_ast::TyKind::ImplTrait(_, _) | // also this
        rustc_ast::TyKind::Never |
        rustc_ast::TyKind::UnsafeBinder(_) |
        rustc_ast::TyKind::FnPtr(_) |
        rustc_ast::TyKind::PinnedRef(_, _) |
        rustc_ast::TyKind::Ptr(_) => {
            todo!("I still don't really know what to do with these types");
            // they are either weird to include for the current use case, or just won't be supported
        },

        // we are trying to get a well formed type string.
        // encountering this means thats impossible
        rustc_ast::TyKind::Infer => panic!(),
    }
}
