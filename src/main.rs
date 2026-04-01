/* Entry point file for DATIR.
 * This file creates and orchestrates the multiple compiler invocations
 * required to perform abstract type inference. The first compilation
 * gathers necessary information about the source code (namely some type
 * information), which the second compilation uses to actually mutate the
 * AST and to add in dynamic instrumentation.
*/
#![feature(rustc_private)]
#![feature(box_patterns)]
#![feature(min_specialization)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;

use std::{env, sync::Arc};

use crate::common::DatirConfig;

// included so VsCode's rust-analyzer extension runs static analysis on the runtime library
mod ati;

mod callbacks;
mod common;
mod file_loaders;
mod types;
mod visitors;

/// Entry-point, forwards all command-line arguments to rustc_driver
pub fn main() {
    let mut args: Vec<_> = env::args().collect();

    // Use debug logging / outputs when invoked via `cargo run -- <root.rs>` for now.
    // FIXME: improve this, I should figure out a better way of passing flags and having the behavior appropriately change
    // it's hard as there are multiple compile invocations, all of which could take different flags
    let config = if args.ends_with(&["TEST_INVOCATION".to_string()]) {
        args.pop();
        Arc::new(DatirConfig::release())
    } else {
        Arc::new(DatirConfig::debug())
    };

    let mut gather_info = callbacks::gather_orig::GatherAtiInfo::new(config.clone());
    rustc_driver::run_compiler(&args, &mut gather_info); // panics on compilation failure
    let first_pass = gather_info.into_first_pass_info();

    let mut cbs =
        callbacks::transform_ast::TransformAbstractSyntaxTreeCallbacks::new(first_pass, config);
    rustc_driver::run_compiler(&args, &mut cbs);
}
