//! Generates shims for free functions.
//!
//! This file contains the logic required to build "shims" for free functions. It's easiest to
//! understand what these shims do by taking a look at an example. Assume the following is a user-
//! defined function, which is currently being compiled:
//! ```rust
//! fn foo(x: u32, y: SomeStruct, z: &SomeStruct) -> (u32, f64) {
//!     // foo's logic...
//! }
//! ```
//!
//! We want to observe comparability information regarding `x`, `y`, `z`, and the `return` at the
//! entrance to this function, and the exit. These are `foo`'s "Program Points" (a.k.a. ppts or
//! sites). This file will generate a "shim function", in place of the original function, which
//! will create the ENTER ppt, bind all parameters to it, and update the comparability information
//! at that ppt. Then, the shim will invoke the original function (the body will be moved to a newly
//! generated function with a non-conflicting, unique name). After it returns, a similar process is
//! used to then create an EXIT ppt, bind only the formals *which are live* at the exit of the
//! original function, and the return value.
//!
//! The above example (also applying instrumentation to the original `foo`, and using some
//! pseudocode) would cause the following functions to be generated:
//! ```rust
//! fn foo(x: Tagged<u32>, y: SomeStruct, z: &SomeStruct) -> (Tagged<u32>, Tagged<f64>) {
//!     let mut enter = ATI_ANALYSIS.get_site("foo:::ENTER");
//!     enter.bind("x", &x);
//!     enter.bind("y", &y);
//!     enter.bind("z", &z);
//!     ATI_ANALYSIS.update_site(enter);
//!
//!     let res = foo0(x, y, z);
//!
//!     let mut exit = ATI_ANALYSIS.get_site("foo:::EXIT");
//!     exit.bind("x", &x);
//!     exit.bind("z", &z);
//!     exit.bind("return", &res);
//!     ATI_ANALYSIS.update_site(exit);
//!
//!     return res;
//! }
//!
//! fn foo0(x: Tagged<u32>, y: SomeStruct, z: &SomeStruct) -> (Tagged<u32>, Tagged<f64>) {
//!     // foo's original logic...
//! }
//! ```
//! It is important that the shim function retains the name of the original function, so that
//! every existing call to this function, calls the shim instead.
//!
//! Note that as `y` was an owned struct, it does not exist at the end of `foo`'s execution,
//! therefore it is not bound to the exit site. Further note the return value, which is bound to
//! the exit site.
//!
//! `.bind()` functionality is dependent on the [SiteBind](crate::ati::site_binds::SiteBind)
//! trait, defined within the runtime library. Implementation of this trait on compound types
//! will result in each recursive field being bound to the site, as a *separate variable*. It's
//! for this reason, that all user-defined compound types have a
//! [SiteBind](crate::ati::site_binds::SiteBind) implementation dynamically generated. Calling
//! `.bind()` on an untracked type will cause a no-op, as there is no known information about
//! that value. Calling `.bind()` on a simple `Tagged<T>` (or reference variant), will only
//! associate the single Id.
//!
//! If the original function does not return anything (or returns unit), then the return value is
//! also ignored. If the original function is main, corresponding ENTER and EXIT sites are still
//! created, but at the end of the function, `.produce_ati()` is invoked (if DATIR is running in
//!  --release mode), or `.report()` otherwise, to actually write comparability output.

use crate::{
    callbacks::codegen::common::{
        build_inner_call_args, create_param_binds, generic_params_to_string, get_param_name,
        get_unique_inner_name, where_clause_to_string,
    },
    callbacks::gather::first_pass_info::{FirstPassInfo, FnNamespace},
    callbacks::parsing,
    config::DatirConfig,
};

use decls_gen::decls::RETURN_VAR_NAME;

/// Generates a shim for a single function.
/// 
/// Shims perform site management, take a look at the header comment on this file to see
/// what that involves.
pub fn generate_function_shim(
    datir_config: &DatirConfig,
    first_pass: &FirstPassInfo,
    psess: &rustc_session::parse::ParseSess,
    mod_path: &str,
    func: &mut Box<rustc_ast::Fn>,
    new_items: &mut Vec<Box<rustc_ast::Item>>,
) {
    let box rustc_ast::Fn {
        ident,
        generics,
        sig: rustc_ast::FnSig { decl, .. },
        body,
        ..
    } = func;

    // find a name for the function which does not conflict with
    // any other name in the current module namespace.
    let orig_name = ident.as_str().to_string();
    let known_names = first_pass.fns.names_in(mod_path, FnNamespace::Free);
    let inner_name = get_unique_inner_name(&orig_name, &known_names);
    if datir_config.print_function_signatures {
        datir_config.log(
            "FunctionStubs",
            format!(
                "Generating free function stub for {mod_path}: {:?} -> {:?}",
                orig_name, inner_name
            ),
        );
    }

    // find the name of the base program point name, discovered in the first pass
    let entry = first_pass
        .fns
        .lookup(mod_path, FnNamespace::Free, ident.as_str())
        .unwrap_or_else(|| {
            panic!(
                "stub generation could not find a FnBasePptName for free fn \
                             `{orig_name}` in module `{mod_path}`"
            )
        });

    // rip the original body out of the free function.
    let orig_body = body
        .take()
        .unwrap_or_else(|| panic!("free fn `{orig_name}` in module `{mod_path}` has no body."));

    // construct fn item that will at some point contain the original body
    let inner_template = build_inner_fn_template(&inner_name, generics, &decl.inputs, &decl.output);
    let mut parsed_items = parsing::parse_items(psess, inner_template, None);
    let mut inner_item = parsed_items
        .pop()
        .expect("inner-fn template did not parse into an item");

    // get a mutable reference to the empty body and place the original
    // body in there!
    let rustc_ast::ItemKind::Fn(box rustc_ast::Fn {
        body: ref mut inner_body,
        ..
    }) = inner_item.kind
    else {
        panic!("inner-fn template did not yield ItemKind::Fn");
    };
    *inner_body = Some(orig_body);

    // we will have to add this inner function item into the current
    // module
    new_items.push(inner_item);

    // EXIT-ppt liveness will determine which formals get bound at the
    // exit site. Pass 1 already validated existence of both ppts.
    let enter_ppt = datir_config
        .decls_file
        .enter_ppt(entry)
        .expect("ENTER ppt missing.");
    let exit_ppt = datir_config
        .decls_file
        .exit_ppt(entry)
        .expect("EXIT ppt missing");

    // construct the "shim code", and insert it where the original body was.
    let wrapper_src = build_fn_wrapper_block(
        datir_config,
        entry,
        &orig_name,
        &inner_name,
        &decl.inputs,
        &decl.output,
        enter_ppt,
        exit_ppt,
    );

    let parsed_wrapper = parsing::parse_expr(psess, wrapper_src);
    let rustc_ast::ExprKind::Block(new_block, _) = parsed_wrapper.kind else {
        panic!("wrapper-block source for free fn `{orig_name}` did not parse as a block");
    };
    *body = Some(new_block);
}

/// Source for an inner free function signature with an empty placeholder body. 
/// 
/// The caller parses this template, then transplants the user's original body into this, to 
/// create an "inner" function which holds the original function's logic.
fn build_inner_fn_template(
    inner_name: &str,
    generics: &rustc_ast::Generics,
    inputs: &[rustc_ast::Param],
    output: &rustc_ast::FnRetTy,
) -> String {
    let generic_params = generic_params_to_string(generics);
    let where_clause = where_clause_to_string(generics);
    let declared = inputs
        .iter()
        .map(|p| {
            // if p was a mutable ref, we had to make sure the emitted
            // formal is declared to be a mut binding... p was translated to
            // be a TaggedRefMut in this case... But also any struct that carried a mut ref
            // also needs to allow mutable access!! just make everything mutable?
            // FIXME: i honestly think this system sucks. not sure how to avoid it
            // without changing the ref (specifically mut ref) implementation again...
            format!(
                "mut {}: {}",
                get_param_name(p),
                rustc_ast_pretty::pprust::ty_to_string(&p.ty)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let ret = match output {
        rustc_ast::FnRetTy::Ty(t) => format!(" -> {}", rustc_ast_pretty::pprust::ty_to_string(t)),
        rustc_ast::FnRetTy::Default(_) => String::new(),
    };
    format!("fn {inner_name}{generic_params}({declared}){ret}{where_clause} {{ }}")
}

/// Creates the body for a free fn's shim. 
/// 
/// Each shim will:
/// 1. open ENTER, bind every formal, update;
/// 2. call `inner_name(args)`;
/// 3. open EXIT, bind only formals still live at exit (per the EXIT ppt's
///    `is_uninit()` tags) and the return value when non-unit, update.
///
/// Special-cased for fn_name == "main": no param binds, no return value to
/// bind, and the analysis report is produced after the EXIT site update.
fn build_fn_wrapper_block(
    config: &DatirConfig,
    base_ppt_name: &str,
    fn_name: &str,
    inner_name: &str,
    inputs: &[rustc_ast::Param],
    output: &rustc_ast::FnRetTy,
    enter_ppt: &decls_gen::ProgramPoint,
    exit_ppt: &decls_gen::ProgramPoint,
) -> String {
    let passed = build_inner_call_args(inputs.iter());
    let enter_binds = create_param_binds("site_enter", inputs.iter(), enter_ppt).join("\n");
    let exit_binds = create_param_binds("site_exit", inputs.iter(), exit_ppt).join("\n");

    if fn_name == "main" {
        // In --release mode each execution produces a fresh .ati file at
        // {ati_output_dir}/{rand:016x}.ati so concurrent or repeated runs
        // don't clobber each other. The dir was wiped + canonicalized in
        // datir's main.rs, so the path here is absolute. Outside release
        // mode just dump the report to stdout via the existing API.
        let post = match &config.ati_output_dir {
            Some(dir) => format!(
                r#"{{
                    let __ati_rand: u64 = std::random::random(..);
                    let __ati_path = format!(r"{}/{{:016x}}.ati", __ati_rand);
                    ATI_ANALYSIS.lock().unwrap().produce_ati(&__ati_path);
                }}"#,
                dir.to_str().expect("ati_output_dir is not valid UTF-8"),
            ),
            None => "ATI_ANALYSIS.lock().unwrap().report();".to_string(),
        };

        return format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {inner_name}();

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                {post}
            }}"#
        );
    }

    match output {
        rustc_ast::FnRetTy::Ty(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let res = {inner_name}({passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                res.bind(&mut site_exit, "{RETURN_VAR_NAME}");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                return res;
            }}"#
        ),
        rustc_ast::FnRetTy::Default(_) => format!(
            r#"{{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::ENTER");
                {enter_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {inner_name}({passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(r"{base_ppt_name}:::EXIT");
                {exit_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
            }}"#
        ),
    }
}
