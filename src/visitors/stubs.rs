/* Walks the already-transformed AST to generate function stubs and BindToSite impls.
 *
 * Iterates krate.items directly so that it can read AST nodes by reference
 * (params, types, fields) while generating stub code strings, without cloning
 * any AST nodes.
*/
use rustc_ast::{self as ast};
use rustc_session::parse::ParseSess;
use rustc_span::Ident;

use crate::common::{self, DatirConfig, parsing};
use crate::types::ati_info::{FirstPassInfo};
use std::collections::{HashMap, HashSet};

/// specifically a string that is an invalid name for a struct or enum.
const REGULAR_FUNCTION_NAMESPACE: &'static str = "-REGULAR-";
/// Map of a namespace (the type of Self, or REGULAR_FUNCTION_NAMESPACE) to 
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

/// Walks the crate, renames tracked functions/methods to proper stub names,
/// generates all stub code, and inserts the parsed stubs into the crate
pub fn generate_stubs(
    datir_config: &DatirConfig,
    krate: &mut ast::Crate,
    first_pass: &FirstPassInfo,
    module_path: &str,
    psess: &ParseSess,
) {
    let known_names: KnownNames = find_all_names(krate, first_pass);

    // iterate through krate items once to find all
    // fn/method names ahead of time, to later avoid collisions.
    if datir_config.print_function_signatures {
        datir_config.log("FunctionStubs", format!("Known Funcs in File {module_path}:\n{:#?}\n", known_names));
    }

    let mut stub_code: Vec<String> = Vec::new();
    for item in krate.items.iter_mut() {
        match &mut item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                sig: ast::FnSig { decl, .. },
                ..
            }) => {
                if !first_pass.is_fn_ident_tracked(ident) {
                    continue;
                }

                let orig_name = ident.as_str().to_string();
                let new_name = get_unique_inner_name(None, &orig_name, &known_names);
                if datir_config.print_function_signatures {
                    datir_config.log("FunctionStubs", format!("Fn Stub: {:#?}", new_name));
                }

                stub_code.push(create_fn_stub(
                    module_path,
                    &orig_name,
                    &new_name,
                    &decl.inputs,
                    &decl.output,
                ));

                *ident = Ident::from_str(&new_name);
            }

            ast::ItemKind::Struct(ident, _, ast::VariantData::Struct { fields, .. }) => {
                stub_code.push(create_struct_bind_impl(ident.as_str(), fields));
            }

            ast::ItemKind::Enum(ident, _, ast::EnumDef { variants }) => {
                stub_code.push(create_enum_bind_impl(ident.as_str(), variants));
            }

            ast::ItemKind::Impl(ast::Impl {
                self_ty, items, ..
            }) => {
                let type_name = common::get_type_string(self_ty);
                // separate method stubs vec to only construct a single
                // impl which contains all of the new stub functions
                let mut method_stubs: Vec<String> = Vec::new();

                for assoc_item in items.iter_mut() {
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        sig: ast::FnSig { decl, .. },
                        ..
                    }) = &mut assoc_item.kind
                    else {
                        continue;
                    };

                    if !first_pass.is_fn_ident_tracked(ident) {
                        continue;
                    }

                    let orig_name = ident.as_str().to_string();
                    let new_name = get_unique_inner_name(Some(&type_name), &orig_name, &known_names);
                    if datir_config.print_function_signatures {
                        datir_config.log("FunctionStubs", format!("Method Stub: {:#?}::{:#?}", type_name, new_name));
                    }

                    method_stubs.push(create_method_stub(
                        module_path,
                        &type_name,
                        &orig_name,
                        &new_name,
                        &decl.inputs,
                        &decl.output,
                    ));

                    *ident = Ident::from_str(&new_name);
                }

                if !method_stubs.is_empty() {
                    stub_code.push(format!(
                        "impl {type_name} {{\n{}\n}}",
                        method_stubs.join("\n\n")
                    ));
                }
            }

            _ => {}
        }
    }

    for code in stub_code {
        for item in parsing::parse_items(psess, code, None) {
            krate.items.insert(0, item);
        }
    }
}

////////////////// Helpers ///////////////////////////

/// Finds the names of all functions that require stubs
fn find_all_names(krate: &ast::Crate, first_pass: &FirstPassInfo) -> KnownNames {
    let mut known_names: KnownNames = HashMap::new();

    for item in krate.items.iter() {
        match &item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                ..
            }) => {
                if !first_pass.is_fn_ident_tracked(ident) {
                    continue;
                }

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

                    if !first_pass.is_fn_ident_tracked(ident) {
                        continue;
                    }

                    let self_ty = common::get_type_string(self_ty);
                    known_names.entry(self_ty).or_default().insert(ident.as_str().to_string());
                }
            }

            _ => {}
        }
    }

    known_names

}

fn qualified_site_name(module_path: &str, name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{module_path}::{name}")
    }
}

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

fn determine_receiver_kind(params: &[ast::Param]) -> ReceiverKind {
    // is the self parameter always the first one?
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

fn create_fn_stub(
    module_path: &str,
    fn_name: &str,
    inner_name: &str,
    inputs: &[ast::Param],
    output: &ast::FnRetTy,
) -> String {
    let site_name = qualified_site_name(module_path, fn_name);

    let (declared_params, passed_params): (Vec<String>, Vec<String>) = inputs
        .iter()
        .map(|param| {
            let name = get_param_name(param);
            let ptype = common::get_type_string(&param.ty);
            (format!("{name}: {ptype}"), name.to_string())
        })
        .unzip();

    let declared = declared_params.join(", ");
    let passed = passed_params.join(", ");
    let all_params = inputs.iter();
    let enter_binds = create_param_binds("site_enter", all_params.clone()).join("\n");
    let exit_binds = create_param_binds("site_exit", all_params).join("\n");

    if fn_name == "main" {
        // TODO: environment stuff for main
        return format!(
            r#"
            pub fn main() {{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                {inner_name}();

                ATI_ANALYSIS.lock().unwrap().report();
            }}
        "#
        );
    }

    match output {
        ast::FnRetTy::Ty(ret_ty) => {
            let ret = common::get_type_string(ret_ty);
            format!(
                r#"
                pub fn {fn_name}({declared}) -> {ret} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    let res = {inner_name}({passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
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
                pub fn {fn_name}({declared}) {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {inner_name}({passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                }}
            "#
            )
        }
    }
}

fn create_method_stub(
    module_path: &str,
    type_name: &str,
    method_name: &str,
    inner_name: &str,
    all_inputs: &[ast::Param],
    output: &ast::FnRetTy,
) -> String {
    let qualified_name = qualified_site_name(module_path, &format!("{type_name}::{method_name}"));

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
            let ptype = common::get_type_string(&param.ty);
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
            let ret = common::get_type_string(ret_ty);
            format!(
                r#"
                pub fn {method_name}({declared_params}) -> {ret} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    let res = {call_expr};

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::EXIT");
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
                pub fn {method_name}({declared_params}) {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::ENTER");
                    {enter_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::EXIT");
                    {exit_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {call_expr};

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{qualified_name}::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                }}
                "#
            )
        }
    }
}

fn create_struct_bind_impl(struct_name: &str, fields: &[ast::FieldDef]) -> String {
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

    format!(
        r#"
        impl BindToSite for {struct_name} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                {bind_calls}
            }}
        }}
        impl BindToSite for &{struct_name} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}

fn create_enum_bind_impl(enum_name: &str, variants: &[ast::Variant]) -> String {
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

    format!(
        r#"
        impl BindToSite for {enum_name} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                match self {{
                    {arms_str}
                }}
            }}
        }}
        impl BindToSite for &{enum_name} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    )
}
