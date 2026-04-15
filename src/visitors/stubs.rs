/* Walks the already-transformed AST to generate function stubs and BindToSite impls
 * to perform ENTER/EXIT site management.
 * 
 * Importantly, all instrumented functions are renamed to a new "inner" name,
 * the generated stub will be called the original function name, and then invoke
 * this inner function. Name conflicts are avoided by scanning over all user-defined
 * functions and methods, and then finding a suffix to append to the inner name
 * that makes it unique.
*/
use rustc_ast::{self as ast};
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_span::Ident;

use crate::common::{DatirConfig, parsing};
use std::collections::{HashMap, HashSet};

/// The namespace that free functions are defined under.
/// This is specifically a string that is an invalid name for a struct or enum.
const REGULAR_FUNCTION_NAMESPACE: &'static str = "-REGULAR-";

/// Map of a namespace (the type of `Self`, or `REGULAR_FUNCTION_NAMESPACE`) to 
/// a set of all methods/functions defined in that namespace.
type KnownNames = HashMap<String, HashSet<String>>;

/// Describes how a method receives its `self` argument
#[derive(Debug)]
pub enum ReceiverKind {
    /// Associated function with no self parameter.
    None,
    /// Takes self
    Value,
    /// Takes &self
    Ref,
    /// Takes &mut self
    RefMut,
}

/// Walks the crate, renames instrumented functions/methods to have "stub" names,
/// generates all stub code, and inserts the parsed stubs into the crate
pub fn generate_stubs(
    datir_config: &DatirConfig,
    krate: &mut ast::Crate,
    module_path: &str,
    psess: &ParseSess,
) {
    // iterate through krate items once to find all
    // fn/method names ahead of time, to later avoid collisions.
    let known_names: KnownNames = find_all_names(krate);
    if datir_config.print_function_signatures {
        datir_config.log("FunctionStubs", format!("Known Funcs in File {module_path}:\n{:#?}\n", known_names));
    }

    let mut stub_code: Vec<String> = Vec::new();
    for item in krate.items.iter_mut() {
        match &mut item.kind {
            // we found a free function!
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                generics,
                sig: ast::FnSig { decl, .. },
                ..
            }) => {
                // generate a non-clashing inner name based off the original name
                let orig_name = ident.as_str().to_string();
                let new_name = get_unique_inner_name(None, &orig_name, &known_names);
                if datir_config.print_function_signatures {
                    datir_config.log("FunctionStubs", format!("Fn Stub: {:#?}", new_name));
                }

                // create a function stub for this function, to be added to the crate later
                stub_code.push(create_fn_stub(
                    datir_config,
                    module_path,
                    &orig_name,
                    &new_name,
                    &decl.inputs,
                    &decl.output,
                    generics,
                ));

                // rename the original function to the inner name
                *ident = Ident::from_str(&new_name);
            }

            // we found a struct definition
            ast::ItemKind::Struct(ident, generics, ast::VariantData::Struct { fields, .. }) => {
                // structs, when passed between function boundaries need to be bind-ed to
                // sites. This requires implementing the BindToSite trait on them, defined
                // in the runtime library.
                stub_code.push(create_struct_bind_impl(ident.as_str(), fields, generics));
            }

            // we found an enum definition
            ast::ItemKind::Enum(ident, generics, ast::EnumDef { variants }) => {
                // similar to structs, enums require the BindToSite trait to be impled as well
                stub_code.push(create_enum_bind_impl(ident.as_str(), variants, generics));
            }

            // we found an impl block, defining methods on some type `self_ty`
            ast::ItemKind::Impl(ast::Impl {
                generics: impl_generics,
                self_ty, items, ..
            }) => {
                let type_name = pprust::ty_to_string(self_ty);
                let bare_type_name = bare_type_name(self_ty).unwrap_or_else(|| type_name.clone());
                let impl_generic_params = generic_params_to_string(impl_generics);
                let impl_where_clause = where_clause_to_string(impl_generics);

                // separate method stubs vec to only construct a single
                // impl which contains all of the new stub functions
                let mut method_stubs: Vec<String> = Vec::new();

                // iterate through all methods defined in this impl block
                // and perform a similar transformation as the one done for 
                // free functions above.
                for assoc_item in items.iter_mut() {
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        generics: method_generics,
                        sig: ast::FnSig { decl, .. },
                        ..
                    }) = &mut assoc_item.kind
                    else {
                        continue;
                    };

                    let orig_name = ident.as_str().to_string();
                    let new_name = get_unique_inner_name(Some(&type_name), &orig_name, &known_names);
                    if datir_config.print_function_signatures {
                        datir_config.log("FunctionStubs", format!("Method Stub: {:#?}::{:#?}", type_name, new_name));
                    }

                    method_stubs.push(create_method_stub(
                        module_path,
                        &bare_type_name,
                        &orig_name,
                        &new_name,
                        &decl.inputs,
                        &decl.output,
                        method_generics,
                    ));

                    *ident = Ident::from_str(&new_name);
                }

                if !method_stubs.is_empty() {
                    stub_code.push(format!(
                        "impl{impl_generic_params} {type_name}{impl_where_clause} {{\n{}\n}}",
                        method_stubs.join("\n\n")
                    ));
                }
            }

            _ => {}
        }
    }

    // actually add all the code to the crate
    for code in stub_code {
        for item in parsing::parse_items(psess, code, None) {
            krate.items.insert(0, item);
        }
    }
}

////////////////// Helpers ///////////////////////////

/// Extracts the bare name (without generic args) from a type that is a path,
/// using the last segment's identifier (e.g., `MyStruct<A, B>` -> `MyStruct`).
fn bare_type_name(ty: &ast::Ty) -> Option<String> {
    let ast::TyKind::Path(_, path) = &ty.kind else {
        return None;
    };
    Some(path.segments.last()?.ident.as_str().to_string())
}

/// Converts the generic params to a string like `<T, U: Clone, 'a>`.
/// Returns an empty string if there are no generic params.
fn generic_params_to_string(generics: &ast::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }

    let params: Vec<String> = generics.params.iter().map(|param| {
        match &param.kind {
            ast::GenericParamKind::Lifetime => {
                let name = format!("'{}", param.ident.as_str());
                if param.bounds.is_empty() {
                    name
                } else {
                    format!("{}: {}", name, pprust::bounds_to_string(&param.bounds))
                }
            }
            ast::GenericParamKind::Type { default } => {
                let mut s = param.ident.as_str().to_string();
                if !param.bounds.is_empty() {
                    s.push_str(&format!(": {}", pprust::bounds_to_string(&param.bounds)));
                }
                if let Some(ty) = default {
                    s.push_str(&format!(" = {}", pprust::ty_to_string(ty)));
                }
                s
            }
            ast::GenericParamKind::Const { ty, default, .. } => {
                let mut s = format!("const {}: {}", param.ident.as_str(), pprust::ty_to_string(ty));
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", pprust::expr_to_string(&d.value)));
                }
                s
            }
        }
    }).collect();

    format!("<{}>", params.join(", "))
}

/// Converts the generic params to a string like `<T, U, 'a>` containing only
/// the names of each parameter (no bounds or defaults). Returns an empty
/// string if there are no generic params.
fn generic_args_to_string(generics: &ast::Generics) -> String {
    if generics.params.is_empty() {
        return String::new();
    }

    let args: Vec<String> = generics.params.iter().map(|param| {
        match &param.kind {
            ast::GenericParamKind::Lifetime => format!("'{}", param.ident.as_str()),
            ast::GenericParamKind::Type { .. } => param.ident.as_str().to_string(),
            ast::GenericParamKind::Const { .. } => param.ident.as_str().to_string(),
        }
    }).collect();

    format!("<{}>", args.join(", "))
}

/// Converts a where clause to a string like ` where T: Clone, U: Send`.
/// Returns an empty string if the where clause is empty.
fn where_clause_to_string(generics: &ast::Generics) -> String {
    if generics.where_clause.predicates.is_empty() {
        return String::new();
    }

    let preds: Vec<String> = generics.where_clause.predicates.iter().map(|pred| {
        match &pred.kind {
            ast::WherePredicateKind::BoundPredicate(bp) => {
                pprust::where_bound_predicate_to_string(bp)
            }
            ast::WherePredicateKind::RegionPredicate(rp) => {
                let lifetime = format!("'{}", rp.lifetime.ident.as_str());
                if rp.bounds.is_empty() {
                    lifetime
                } else {
                    format!("{}: {}", lifetime, pprust::bounds_to_string(&rp.bounds))
                }
            }
            ast::WherePredicateKind::EqPredicate(ep) => {
                unreachable!("Found unsupported EqPredicate in where clause")
            }
        }
    }).collect();

    format!(" where {}", preds.join(", "))
}

/// Finds the names of all functions that require stubs
fn find_all_names(krate: &ast::Crate) -> KnownNames {
    let mut known_names: KnownNames = HashMap::new();

    for item in krate.items.iter() {
        match &item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                ..
            }) => {
                known_names.entry(REGULAR_FUNCTION_NAMESPACE.to_string()).or_default().insert(ident.as_str().to_string());
            }

            ast::ItemKind::Impl(ast::Impl {
                self_ty, items, ..
            }) => {
                for assoc_item in items.iter() {
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        ..
                    }) = &assoc_item.kind
                    else {
                        continue;
                    };


                    let self_ty = pprust::ty_to_string(self_ty);
                    known_names.entry(self_ty).or_default().insert(ident.as_str().to_string());
                }
            }

            _ => {}
        }
    }

    known_names

}

/// Creates a unique site name based off the file the function is defined in,
/// alongside the name of the function that this site corresponds to.
fn qualified_site_name(module_path: &str, name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{module_path}.{name}")
    }
}

/// Creates an inner name that does not clash with any other function/method 
/// defined in the file.
fn get_unique_inner_name(namespace: Option<&str>, original: &str, known_names: &KnownNames) -> String {
    let Some(known) = (match namespace {
        Some(type_name) =>  {
            known_names.get(type_name)
        },
        None => {
            known_names.get(REGULAR_FUNCTION_NAMESPACE)
        },
    }) else {
        panic!("Attempting to generate stub name given an unknown namespace: {namespace:?}");
    };

    let mut suffix = 0;
    let mut candidate = format!("{original}{suffix}");
    while known.contains(&candidate) {
        suffix += 1;
        candidate = format!("{original}{suffix}");
    }

    candidate
}

/// Determines whether a list of parameters being passed to some
/// function or method accepts self, &self, &mut self.
fn determine_receiver_kind(params: &[ast::Param]) -> ReceiverKind {
    // can the self parameter be something other than the first param?
    let Some(first) = params.first() else {
        return ReceiverKind::None;
    };

    match &first.ty.kind {
        ast::TyKind::ImplicitSelf => ReceiverKind::Value,
        ast::TyKind::Ref(_, ast::MutTy { mutbl, .. }) => {
            if matches!(mutbl, ast::Mutability::Mut) {
                ReceiverKind::RefMut
            } else {
                ReceiverKind::Ref
            }
        }
        _ => ReceiverKind::None,
    }
}

/// gets the name of a parameter passed to some function
// FIXME: I'm not sure why using pprust::pat_to_string(param.pat) instead causes a panic?
fn get_param_name(param: &ast::Param) -> String {
    match param.pat.kind {
        rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
        _ => unreachable!("Cannot get name of non-Ident param name"),
    }
}

/// Generates bind statements for parameters against a site variable.
fn create_param_binds<'a>(site_name: &str, params: impl Iterator<Item=&'a ast::Param>) -> Vec<String> {
    params
        .filter(|param| {
            matches!(
                &param.ty.kind,
                ast::TyKind::Array(_, _)
                    | ast::TyKind::Slice(_)
                    | ast::TyKind::Ref(_, _)
                    | ast::TyKind::Tup(_)
                    | ast::TyKind::Path(_, _)
            )
        })
        .map(|param| {
            let var_name = get_param_name(param);
            format!(r#"{var_name}.bind(&mut {site_name}, "{var_name}");"#)
        })
        .collect()
}

// ========== Stub generation ==========

/// Creates a stub for a free function using the passed in information.
/// 
/// A function stub will create an ENTER site, with each input parameter bound to it, and 
/// an EXIT site with each input parameter and the return value bound. Between these two 
/// sites, the corresponding inner function is invoked. Importantly, the stub has the 
/// same original name of the inner function, which means any invocation of that function
/// will now invoke this stub instead.
/// 
/// Abstract Type information about the inputs and outputs is reported at these locations.
fn create_fn_stub(
    config: &DatirConfig,
    module_path: &str,
    fn_name: &str,
    inner_name: &str,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
    generics: &ast::Generics,
) -> String {
    let site_name = qualified_site_name(module_path, fn_name);
    let generic_params = generic_params_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    let (declared_params, passed_params): (Vec<String>, Vec<String>) = inputs
        .iter()
        .map(|param| {
            let name = get_param_name(param);
            let ptype = pprust::ty_to_string(&param.ty);
            (format!("{name}: {ptype}"), name.to_string())
        })
        .unzip();

    let declared = declared_params.join(", ");
    let passed = passed_params.join(", ");
    let all_params = inputs.iter();
    let enter_binds = create_param_binds("site_enter", all_params.clone()).join("\n");
    let exit_binds = create_param_binds("site_exit", all_params).join("\n");

    if fn_name == "main" {
        let report_fmt = if let Some(output_file_name) = &config.output_decls_format {
            format!("produce_decls(\"{}\")", output_file_name.to_str().unwrap())
        } else {
            "report()".to_string()
        };

        // FIXME: environment stuff for main
        return format!(
            r#"
            pub fn main() {{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::ENTER");
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::EXIT");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                {inner_name}();

                ATI_ANALYSIS.lock().unwrap().{report_fmt};
            }}
        "#
        );
    }

    match output {
        ast::FnRetTy::Ty(ret_ty) => {
            let ret = pprust::ty_to_string(ret_ty);
            format!(
                r#"
                pub fn {fn_name}{generic_params}({declared}) -> {ret}{where_clause} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    let res = {inner_name}({passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::EXIT");
                    res.bind(&mut site_exit, "RET");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    return res;
                }}
            "#
            )
        }
        ast::FnRetTy::Default(_) => {
            format!(
                r#"
                pub fn {fn_name}{generic_params}({declared}){where_clause} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {inner_name}({passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}:::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                }}
            "#
            )
        }
    }
}

/// Similar to create_fn_stub, this function instead creates a stub for a method defined on some
/// type. See `create_fn_stub` for more information.
fn create_method_stub(
    module_path: &str,
    type_name: &str,
    method_name: &str,
    inner_name: &str,
    all_inputs: &[ast::Param],
    output: &ast::FnRetTy,
    generics: &ast::Generics,
) -> String {
    let qualified_name = qualified_site_name(module_path, &format!("{type_name}.{method_name}"));
    let generic_params = generic_params_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    let receiver = determine_receiver_kind(all_inputs);

    let mut non_self_inputs = all_inputs.iter();
    let receiver_decl = match receiver {
        ReceiverKind::None => "",
        ReceiverKind::Value => {
            non_self_inputs.next();
            "self"
        }
        ReceiverKind::Ref => {
            non_self_inputs.next();
            "&self"
        }
        ReceiverKind::RefMut => {
            non_self_inputs.next();
            "&mut self"
        },
    };

    let (other_declared, other_passed): (Vec<String>, Vec<String>) = non_self_inputs.clone()
        .map(|param| {
            let name = get_param_name(param);
            let ptype = pprust::ty_to_string(&param.ty);
            (format!("{name}: {ptype}"), name.to_string())
        })
        .unzip();

    // as input paramsinclude optional self parameter, followed by all declared parameters
    let declared_params = match (receiver_decl.is_empty(), other_declared.is_empty()) {
        (true, _) => other_declared.join(", "),
        (false, true) => receiver_decl.to_string(),
        (false, false) => format!("{receiver_decl}, {}", other_declared.join(", ")),
    };

    let passed_params = other_passed.join(", ");

    let call_expr = match receiver {
        ReceiverKind::None => format!("Self::{inner_name}({passed_params})"),
        _ => format!("self.{inner_name}({passed_params})"),
    };

    let self_bind = |site_name: &str| -> String {
        match receiver {
            ReceiverKind::None => String::new(),
            ReceiverKind::Value => format!(r#"self.bind(&mut {site_name}, "self");"#),
            ReceiverKind::Ref | ReceiverKind::RefMut => {
                format!(r#"(*self).bind(&mut {site_name}, "self");"#)
            }
        }
    };

    let enter_binds = [self_bind("site_enter")]
        .into_iter()
        .chain(create_param_binds("site_enter", non_self_inputs.clone()))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let exit_binds = [self_bind("site_exit")]
        .into_iter()
        .chain(create_param_binds("site_exit", non_self_inputs))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    match output {
        ast::FnRetTy::Ty(ret_ty) => {
            let ret = pprust::ty_to_string(ret_ty);
            format!(
                r#"
                pub fn {method_name}{generic_params}({declared_params}) -> {ret}{where_clause} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    let res = {call_expr};

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::EXIT");
                    res.bind(&mut site_exit, "RET");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    return res;
                }}
                "#
            )
        }
        ast::FnRetTy::Default(_) => {
            format!(
                r#"
                pub fn {method_name}{generic_params}({declared_params}){where_clause} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {call_expr};

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}:::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                }}
                "#
            )
        }
    }
}

/// Implements the BindToSite trait (defined in the runtime library) on some struct.
/// 
/// This allows calling struct.bind(site) to recursively associated all fields of the
/// struct with the passed in site. Stub functions rely on this to add all relevant 
/// values to sites.
fn create_struct_bind_impl(
    struct_name: &str,
    fields: &[ast::FieldDef],
    generics: &ast::Generics,
) -> String {
    let bind_calls = fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref()?.as_str();
            Some(format!(
                r#"self.{field_name}.bind(site, &format!("{{var_name}}.{field_name}"));"#
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let generic_params = generic_params_to_string(generics);
    let generic_args = generic_args_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    format!(
        r#"
        impl{generic_params} BindToSite for {struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                {bind_calls}
            }}
        }}
        impl{generic_params} BindToSite for &{struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}

/// Implements the BindToSite trait on some Enum. See comment for `create_struct_bind_impl`.
fn create_enum_bind_impl(
    enum_name: &str,
    variants: &[ast::Variant],
    generics: &ast::Generics,
) -> String {
    let arms: Vec<String> = variants
        .iter()
        .map(|variant| {
            let vname = variant.ident.as_str();
            match &variant.data {
                ast::VariantData::Unit(_) => {
                    format!("{enum_name}::{vname} => {{}}")
                }
                ast::VariantData::Tuple(fields, _) => {
                    let vars: Vec<String> =
                        (0..fields.len()).map(|i| format!("f{i}")).collect();
                    let pattern = vars.join(", ");
                    let binds = vars
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            format!(
                                r#"(*{v}).bind(site, &format!("{{var_name}}<<{vname}>>.{i}"));"#
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{enum_name}::{vname}({pattern}) => {{ {binds} }}")
                }
                ast::VariantData::Struct { fields, .. } => {
                    let field_names: Vec<&str> = fields
                        .iter()
                        .filter_map(|f| Some(f.ident.as_ref()?.as_str()))
                        .collect();
                    let pattern = field_names.join(", ");
                    let binds = field_names
                        .iter()
                        .map(|name| {
                            format!(
                                r#"(*{name}).bind(site, &format!("{{var_name}}<<{vname}>>.{name}"));"#
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{enum_name}::{vname} {{ {pattern} }} => {{ {binds} }}")
                }
            }
        })
        .collect();

    let arms_str = arms.join("\n");

    let generic_params = generic_params_to_string(generics);
    let generic_args = generic_args_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    format!(
        r#"
        impl{generic_params} BindToSite for {enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                match self {{
                    {arms_str}
                }}
            }}
        }}
        impl{generic_params} BindToSite for &{enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}
