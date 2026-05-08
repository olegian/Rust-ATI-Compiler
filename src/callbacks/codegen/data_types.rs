//! Implements the runtime library's [SiteBind](crate::ati::site_binds::SiteBind) trait for
//! user-defined compound types.
//!
//! All values (of both atomic and compound types), must be able to be "bound" to sites. This
//! ultimately just means recording the existing Id associated with some value that is stored
//! within some variable, at a particular site. The [SiteBind](crate::ati::site_binds::SiteBind)
//! trait governs this behavior. Following instrumentation, DATIR generates
//! [SiteBind](crate::ati::site_binds::SiteBind) implementations for all user-defined compound
//! types, so that they recursively bind any tagged fields stored within them.
//!
//! To see an example of how this is actually used by shim functions, look at
//! [crate::callbacks::codegen::function].

use crate::{
    callbacks::codegen::common::{
        generic_args_to_string, generic_params_to_string, where_clause_to_string,
    },
    callbacks::parsing,
};

/// Implements the BindToSite trait on a user-defined struct. 
/// 
/// The generated impl recursively binds each field of
/// the struct against the passed-in site. Stub functions rely on this to
/// add all relevant values to sites.
///
/// Both Struct and Tuple struct flavors are accepted (Unit structs have no
/// fields and produce an empty impl body but are still valid).
pub fn generate_struct_impls(
    psess: &rustc_session::parse::ParseSess,
    struct_name: &str,
    fields: &[rustc_ast::FieldDef],
    generics: &rustc_ast::Generics,
    new_items: &mut Vec<Box<rustc_ast::Item>>,
) {
    let bind_calls = fields
        .iter()
        .enumerate()
        .map(|(i, field)| match &field.ident {
            Some(field_name) => {
                let field_name = field_name.as_str();
                format!(r#"self.{field_name}.bind(site, &format!("{{var_name}}.{field_name}"));"#)
            }
            None => format!(r#"self.{i}.bind(site, &format!("{{var_name}}.{i}"));"#),
        })
        .collect::<Vec<_>>()
        .join("\n");

    let generic_params = generic_params_to_string(generics);
    let generic_args = generic_args_to_string(generics);
    let where_clause = where_clause_to_string(generics);

    let code = format!(
        r#"
        impl{generic_params} SiteBind for {struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                {bind_calls}
            }}
        }}
        impl{generic_params} SiteBind for &{struct_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    );

    for parsed_item in parsing::parse_items(psess, code, None) {
        new_items.push(parsed_item);
    }
}

/// Implements the BindToSite trait on a user-defined enum.
/// 
/// The generated impl matches on the enum and recursively binds each variant's payload
/// against the passed-in site. See [`generate_struct_impls`] for more information.
pub fn generate_enum_impls(
    psess: &rustc_session::parse::ParseSess,
    enum_name: &str,
    variants: &[rustc_ast::Variant],
    generics: &rustc_ast::Generics,
    new_items: &mut Vec<Box<rustc_ast::Item>>,
) {
    let arms: Vec<String> = variants
        .iter()
        .map(|variant| {
            let vname = variant.ident.as_str();
            match &variant.data {
                rustc_ast::VariantData::Unit(_) => {
                    format!("{enum_name}::{vname} => {{}}")
                }
                rustc_ast::VariantData::Tuple(fields, _) => {
                    let vars: Vec<String> = (0..fields.len()).map(|i| format!("f{i}")).collect();
                    let pattern = vars.join(", ");
                    let binds = vars
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            format!(r#"(*{v}).bind(site, &format!("{{var_name}}::{vname}.{i}"));"#)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{enum_name}::{vname}({pattern}) => {{ {binds} }}")
                }
                rustc_ast::VariantData::Struct { fields, .. } => {
                    let field_names: Vec<&str> = fields
                        .iter()
                        .filter_map(|f| Some(f.ident.as_ref()?.as_str()))
                        .collect();
                    let pattern = field_names.join(", ");
                    let binds = field_names
                        .iter()
                        .map(|name| {
                            format!(
                                r#"(*{name}).bind(site, &format!("{{var_name}}::{vname}.{name}"));"#
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

    let code = format!(
        r#"
        impl{generic_params} SiteBind for {enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                match self {{
                    {arms_str}
                }}
            }}
        }}
        impl{generic_params} SiteBind for &{enum_name}{generic_args}{where_clause} {{
            fn bind(&self, site: &mut Site, var_name: &str) {{
                (**self).bind(site, var_name);
            }}
        }}
        "#
    );

    for parsed_item in parsing::parse_items(psess, code, None) {
        new_items.push(parsed_item);
    }
}
