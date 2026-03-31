/* Because we are invoking the compiler multiple times, we need some
 * way of relaying information between the multiple compilations. This file
 * defines some structs which can be used for just that.
 *
 * FirstPassInfo is used to relay information from the first pass, which
 * discovers what functions we are going to be instrumenting and where we are
 * making calls to untracked functions.
 *
 * FirstPassInfo is then used to during the second compilation to only
 * instrument specific functions, during which FunctionSignatures is constructed.
 * FunctionSignatures is used to record the updated data types used in function
 * inputs and outputs, as well as the function name and parameter names.
 * FunctionSignatures is then consumed by the stub creation process, to add in
 * the correct stubs responsible for managing sites.
*/

// TODO: This file is due for a refactor. FunctionBoudnaries / FunctionSignatures is doing too much.

use std::collections::{HashMap, HashSet};

use rustc_ast::{FieldDef, Param, ast, ast::Variant};
use rustc_hir::def_id::DefId;
use rustc_middle as mir;
use rustc_session::parse::ParseSess;
use rustc_span::{Ident, Span};

use crate::common::CanBeTupled;
use crate::common::{self, parsing};

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

/// Contains all information that is going to be passed between the
/// first and second compilation rounds. Populated by invoking the
/// compiler, using the GatherAtiInfo callbacks.
#[derive(Debug)]
pub struct FirstPassInfo {
    /// which user-defined functions are instrumented across the entire project
    tracked_fn_def_ids: HashSet<DefId>,
    tracked_fn_idents: HashSet<Ident>,

    /// places where a track_slice needs to be inserted, as a coercion from an array to a slice type occurred
    array_to_slice_locs: HashSet<Span>,

    /// places where a non-tracked function is called
    /// mapped to whether the return type at that call site is tupleable (i.e. a tracked primitive).
    // FIXME: these function calls could return complex types, like structs, which can be tupled but that requires
    // defining a new struct with Tagged variants of all fields, and that's hard to do :(, ignoring for now.
    // hopefully it won't be a problem...
    untracked_fn_calls: HashMap<Span, bool>,
}

impl Default for FirstPassInfo {
    fn default() -> Self {
        Self {
            tracked_fn_def_ids: Default::default(),
            tracked_fn_idents: Default::default(),
            array_to_slice_locs: Default::default(),
            untracked_fn_calls: Default::default(),
        }
    }
}

impl FirstPassInfo {
    /// register that a function with `ident` and `def_id` should
    /// instrumented later
    // NOTE: This is only really useful for extern crates and library files 
    // that we are unable to instrument. For now, there is no reason to do this
    // as we assume that all code 
    pub fn observe_tracked_fn(&mut self, ident: &Ident, def_id: DefId) {
        self.tracked_fn_idents.insert(ident.clone());
        self.tracked_fn_def_ids.insert(def_id);
    }

    /// register that a function call was made to an untracked function at
    /// `loc`, which returned a value of type `ty`
    pub fn observe_untracked_fn_call<'a>(&mut self, loc: Span, ty: mir::ty::Ty<'a>) {
        self.untracked_fn_calls.insert(loc, ty.can_be_tupled());
    }

    /// register that at this `loc`, an array was implicitly coereced to a slice
    /// (which requires going from a Tagged<[T; N]> to a Tagged<&[T]>)
    pub fn observe_slice_coercion(&mut self, loc: Span) {
        self.array_to_slice_locs.insert(loc);
    }

    /// returns true if this identifier represent a tracked function
    pub fn is_fn_ident_tracked(&self, ident: &Ident) -> bool {
        self.tracked_fn_idents.contains(ident)
    }

    /// returns true if this def_id represents a tracked function
    pub fn is_fn_def_id_tracked(&self, def_id: &DefId) -> bool {
        self.tracked_fn_def_ids.contains(def_id)
    }

    /// returns whether the return type of an untracked function call at this
    /// location is tupleable, if such a call exists
    pub fn is_untracked_call_ret_tupleable(&self, location: &Span) -> Option<bool> {
        self.untracked_fn_calls.get(location).copied()
    }

    pub fn is_span_ref_type_coercion(&self, location: &Span) -> bool {
        self.array_to_slice_locs.contains(location)
    }
}

/// This struct is responsible for packaging together the new function signatures
/// of functions that were modified, for which function stubs need to be created.
/// Each stub requires knowledge of the function name, param names + types, and the
/// return type, all of which is encoded in the various maps below.
#[derive(Debug)]
pub struct FunctionSignatures {
    /// The module path for the file being processed (e.g., `""` for root, `"dep"` for dep.rs).
    /// Used to qualify runtime site names so they don't collide across modules.
    module_path: String,

    /// maps fn_name -> ([input params], Option<return type>)
    fn_sigs: HashMap<String, (Vec<ast::Param>, Option<ast::Ty>)>,
    /// maps type_name -> [(method_name, receiver_kind, non-self params, output)].
    /// receiver kind denotes self, &self, &mut self, or no-self
    method_sigs: HashMap<String, Vec<(String, ReceiverKind, Vec<ast::Param>, Option<ast::Ty>)>>,

    /// Maps original function/method names to their unique renamed identifiers.
    /// Avoids collisions when user code already defines e.g. `foo_unstubbed`.
    rename_map: HashMap<String, String>,

    /// All identifiers known in this file (tracked function names). Used to
    /// detect `_unstubbed` naming collisions.
    known_idents: HashSet<String>,

    /// user-defined structs that need Tagged versions created
    def_structs: HashMap<String, Vec<ast::FieldDef>>,
    /// user-defined enums that need Tagged versions created
    def_enums: HashMap<String, Vec<ast::Variant>>,
}

impl Default for FunctionSignatures {
    fn default() -> Self {
        Self {
            module_path: String::new(),
            fn_sigs: HashMap::new(),
            def_structs: HashMap::new(),
            def_enums: HashMap::new(),
            method_sigs: HashMap::new(),
            rename_map: HashMap::new(),
            known_idents: HashSet::new(),
        }
    }
}

impl FunctionSignatures {
    /// Sets the module path used to qualify runtime site names
    pub fn set_module_path(&mut self, module_path: &str) {
        self.module_path = module_path.to_string();
    }

    /// Registers an identifier as known in this file so `_unstubbed`
    /// rename collisions can be detected and avoided
    // FIXME: I honestly think that random name-mangling is a more reasonable
    // and simple appraoch, but this keeps the name somewhat readable and therefore
    // easier to debug.
    pub fn add_known_ident(&mut self, ident: &str) {
        self.known_idents.insert(ident.to_string());
    }

    /// Returns the module-qualified site name for a function or method.
    /// For root files (lib.rs, main.rs), returns just the name.
    /// For dep files, returns `"module_path::name"`.
    fn qualified_site_name(&self, name: &str) -> String {
        if self.module_path.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.module_path, name)
        }
    }

    /// Finds a unique `_unstubbed` name for some `name`d function.
    /// Returns the chosen unstubbed name (e.g., "foo" -> "foo_unstubbed").
    pub fn reserve_unstubbed_name(&mut self, name: &str) -> String {
        self.reserve_unstubbed_name_for(name, name)
    }

    /// Finds a unique `_unstubbed` name for a method, where the
    /// map key differs from the local identifier.
    ///
    /// `map_key` is the lookup key (e.g., `"TypeName::method"`).
    /// `local_name` is the bare identifier used for collision checking
    /// and suffix generation (e.g., `"method"`).
    ///
    /// Returns the chosen local unstubbed name (e.g., "TypeName::method" -> "method_unstubbed").
    pub fn reserve_unstubbed_name_for(&mut self, map_key: &str, local_name: &str) -> String {
        let mut candidate = format!("{local_name}_unstubbed");
        // This is really stupid but should avoid all name collisions.
        // Read above FIXME.
        let mut suffix = 0;
        while self.known_idents.contains(&candidate) {
            suffix += 1;
            candidate = format!("{local_name}_unstubbed_{suffix}");
        }
        self.rename_map.insert(map_key.to_string(), candidate.clone());
        candidate
    }

    /// Looks up the unstubbed name that was assigned to the given map key.
    pub fn get_unstubbed_name(&self, map_key: &str) -> &str {
        self.rename_map
            .get(map_key)
            .expect(&format!("No unstubbed name registered for '{map_key}'"))
    }

    /// Observes a new struct def
    pub fn register_struct_def(&mut self, name: &str, field_defs: &[FieldDef]) {
        self.def_structs.insert(name.into(), field_defs.into());
    }

    /// Observes a new enum def
    pub fn register_enum_def(&mut self, name: &str, variants: &[Variant]) {
        self.def_enums.insert(name.into(), variants.into());
    }

    /// Records a method belonging to `type_name`. `receiver` describes the self
    /// parameter kind; `inputs` contains only the non-self parameters (already
    /// updated to use `Tagged<T>` types)
    pub fn register_method_sig(
        &mut self,
        type_name: &str,
        method_name: &str,
        receiver: ReceiverKind,
        inputs: Vec<&Param>,
        output: Option<&ast::Ty>,
    ) {
        self.method_sigs.entry(type_name.into()).or_default().push((
            method_name.into(),
            receiver,
            inputs.into_iter().cloned().collect(),
            output.cloned(),
        ));
    }

    /// Observes a new function signature, with the given name, inputs, and output
    pub fn register_fn_sig(&mut self, name: &str, inputs: Vec<&Param>, output: Option<&ast::Ty>) {
        self.fn_sigs.insert(
            name.into(),
            (inputs.into_iter().cloned().collect(), output.cloned()),
        );
    }

    /// Add all required stubs, struct/enum definitions, impl blocks to the passed in krate.
    // FIXME: might be able to have this fully consume self
    pub fn create_stub_items(&self, krate: &mut ast::Crate, psess: &ParseSess) {
        // create stubs for functions
        for fn_name in self.fn_sigs.keys() {
            let code = self.create_fn_stub(fn_name);

            for item in parsing::parse_items(psess, code, None) {
                krate.items.insert(0, item);
            }
        }

        // create stubs for methods
        for (type_name, sigs) in &self.method_sigs {
            let code = self.create_impl_stub_block(type_name, sigs);

            for item in parsing::parse_items(psess, code, None) {
                krate.items.insert(0, item);
            }
        }

        // implement required trait to .bind user defined structs to sites
        for struct_name in self.def_structs.keys() {
            let code = self.create_struct_bind_impl(struct_name);

            for item in parsing::parse_items(psess, code, None) {
                krate.items.insert(0, item);
            }
        }

        // do the same for user defined enums
        for enum_name in self.def_enums.keys() {
            let code = self.create_enum_bind_impl(enum_name);

            for item in parsing::parse_items(psess, code, None) {
                krate.items.insert(0, item);
            }
        }
    }

    /// Generates `impl BindToSite for StructName { fn bind(...) }` for a tracked struct,
    /// plus a reference impl `impl BindToSite for &StructName` that delegates via deref.
    fn create_struct_bind_impl(&self, struct_name: &str) -> String {
        let fields = self.def_structs.get(struct_name).unwrap();

        let bind_calls = fields
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?.as_str().to_string();
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

    /// Generates `impl BindToSite for EnumName` with a match over each variant,
    /// plus a reference impl that delegates via deref.
    fn create_enum_bind_impl(&self, enum_name: &str) -> String {
        let variants = self.def_enums.get(enum_name).unwrap();

        let arms: Vec<String> = variants
            .iter()
            .map(|variant| {
                let vname = variant.ident.as_str();
                match &variant.data {
                    // TODO: these are actually a little worrying. Enum variants themselves can be 
                    // compared with one another, does that mean that we shuold put a tag around enum variants
                    // that corresponds to the enum variant's discriminant?
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
                        let field_names: Vec<String> = fields
                            .iter()
                            .filter_map(|f| Some(f.ident.as_ref()?.as_str().to_string()))
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

    /// Generates an `impl TypeName { ... }` block that contains a stub for every
    /// tracked method in `sigs`. The stubs retain the original method names so
    /// that call sites (`s.method(...)`) continue to work without modification.
    fn create_impl_stub_block(
        &self,
        type_name: &str,
        sigs: &[(String, ReceiverKind, Vec<ast::Param>, Option<ast::Ty>)],
    ) -> String {
        let stubs: Vec<String> = sigs
            .iter()
            .map(|(method_name, receiver, inputs, output)| {
                self.create_method_stub(type_name, method_name, receiver, inputs, output.as_ref())
            })
            .collect();

        // TODO: THIS SHOULD CREATE STUBS? WHY IS IT NOT?
        format!("impl {type_name} {{\n{}\n}}", stubs.join("\n\n"))
    }

    /// Generates a single stub method for `type_name::method_name`.
    ///
    /// The stub creates ENTER and EXIT sites named `TypeName::method_name::ENTER/EXIT`,
    /// binds all parameters (including struct fields via `self.bind`), then delegates
    /// to the renamed `method_name_unstubbed` for the actual computation.
    fn create_method_stub(
        &self,
        type_name: &str,
        method_name: &str,
        receiver: &ReceiverKind,
        inputs: &[ast::Param],
        output: Option<&ast::Ty>,
    ) -> String {
        let local_qualified = format!("{type_name}::{method_name}");
        let qualified_name = self.qualified_site_name(&local_qualified);
        let unstubbed_name = self.get_unstubbed_name(&format!("{type_name}::{method_name}"));

        // FIXME: this can almost definitely be turned into a to_string kind of thing
        let receiver_decl = match receiver {
            ReceiverKind::None => "",
            ReceiverKind::Value => "self",
            ReceiverKind::Ref => "&self",
            ReceiverKind::RefMut => "&mut self",
        };

        // Declared and passed strings for the NON-SELF parameters.
        let (other_declared, other_passed): (Vec<String>, Vec<String>) = inputs
            .iter()
            .map(|param| {
                let name = self.get_param_name(param);
                let ptype = common::get_type_string(&param.ty);
                (format!("{name}: {ptype}"), name)
            })
            .unzip();

        let declared_params = match (receiver_decl.is_empty(), other_declared.is_empty()) {
            (true, _) => other_declared.join(", "),      // no self parameter
            (false, true) => receiver_decl.to_string(),  // self parameter, with no other params
            (false, false) => format!("{receiver_decl}, {}", other_declared.join(", ")),   // self param and other params
        };

        let passed_params = other_passed.join(", ");

        let call_expr = match receiver {
            ReceiverKind::None => format!("Self::{unstubbed_name}({passed_params})"),
            _ => format!("self.{unstubbed_name}({passed_params})"),
        };

        // Generate site-bind statements. For receivers that have a `self`, also bind
        // the struct/enum fields by delegating to the generated `BindToSite` impl.
        // TODO: extract this to a function, parameterized over the site_name
        let self_bind_enter: String = match receiver {
            ReceiverKind::None => String::new(),
            ReceiverKind::Value => format!(r#"self.bind(&mut site_enter, "self");"#),
            ReceiverKind::Ref | ReceiverKind::RefMut => {
                format!(r#"(*self).bind(&mut site_enter, "self");"#)
            }
        };
        let self_bind_exit: String = match receiver {
            ReceiverKind::None => String::new(),
            ReceiverKind::Value => format!(r#"self.bind(&mut site_exit, "self");"#),
            ReceiverKind::Ref | ReceiverKind::RefMut => {
                format!(r#"(*self).bind(&mut site_exit, "self");"#)
            }
        };

        // FIXME: create parameterized helpers for all of this stuff...
        let mut enter_parts = vec![self_bind_enter];
        enter_parts.extend(self.create_site_binds_for_params("site_enter", inputs));
        let enter_binds = enter_parts
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let mut exit_parts = vec![self_bind_exit];
        exit_parts.extend(self.create_site_binds_for_params("site_exit", inputs));
        let exit_binds = exit_parts
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        if let Some(ret_ty) = output {
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
        } else {
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

    /// Builds site-bind statements for a slice of (already-tagged) parameters.
    /// Mirrors `create_site_binds` but accepts params directly rather than looking
    /// them up from `fn_sigs`.
    fn create_site_binds_for_params(&self, site_name: &str, inputs: &[ast::Param]) -> Vec<String> {
        inputs
            .iter()
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
                let var_name = self.get_param_name(param);
                format!(r#"{var_name}.bind(&mut {site_name}, "{var_name}");"#)
            })
            .collect()
    }

    fn create_fn_stub(&self, fn_name: &str) -> String {
        let (inputs, output) = self
            .fn_sigs
            .get(fn_name)
            .expect("Attempting to create function stub out of non-registered function");

        let (declared_params, passed_params): (Vec<String>, Vec<String>) = inputs
            .iter()
            .map(|param| {
                let name = self.get_param_name(param);
                let ptype = common::get_type_string(&param.ty);

                (format!("{name}: {ptype}"), name)
            })
            .unzip();

        let enter_param_binds = self.create_site_binds("site_enter", fn_name);
        let exit_param_binds = self.create_site_binds("site_exit", fn_name);

        self.create_stub(
            fn_name,
            declared_params.join(", "),
            passed_params.join(", "),
            enter_param_binds.join("\n"),
            exit_param_binds.join("\n"),
            output.as_ref().map(|ty| common::get_type_string(ty)),
        )
    }

    fn get_param_name(&self, param: &ast::Param) -> String {
        match param.pat.kind {
            rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
            _ => {
                unreachable!("Cannot get name of non-Ident param name")
            }
        }
    }

    fn create_site_binds(&self, site_name: &str, fn_name: &str) -> Vec<String> {
        let (inputs, _) = self.fn_sigs.get(fn_name).unwrap();

        // at this point, inputs should have been wrapped in TV<> if possible
        inputs
            .iter()
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
                let var_name = self.get_param_name(param);
                format!(r#"{var_name}.bind(&mut {site_name}, "{var_name}");"#)
            })
            .collect::<Vec<String>>()
    }

    fn create_stub(
        &self,
        fn_name: &str,
        declared_params: String,
        passed_params: String,
        enter_param_binds: String,
        exit_param_binds: String,
        output: Option<String>,
    ) -> String {
        let site_name = self.qualified_site_name(fn_name);
        let unstubbed_name = self.get_unstubbed_name(fn_name);

        if fn_name == "main" {
            // TODO: environment stuff for main
            // this is kind of a silly stub for now...
            format!(
                r#"
                pub fn main() {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {unstubbed_name}();

                    ATI_ANALYSIS.lock().unwrap().report();
                }}
            "#
            )
        } else if let Some(ret) = output {
            // with a return value
            format!(
                r#"
                pub fn {fn_name}({declared_params}) -> {ret} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    {exit_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    let res = {unstubbed_name}({passed_params});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    res.bind(&mut site_exit, "RET");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    return res;
                }}
            "#
            )
        } else {
            // without a return value, still need to perform update on exit though.
            format!(
                r#"
                pub fn {fn_name}({declared_params}) {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::ENTER");
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    {exit_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                    {unstubbed_name}({passed_params});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{site_name}::EXIT");
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                }}
            "#
            )
        }
    }
}
