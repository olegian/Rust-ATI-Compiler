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

// included just for VsCode's rust-analyzer extension to run analysis on the runtime library
mod ati;
mod callbacks;
mod common;
mod file_loaders;
mod types;
mod visitors;

/// Entry-point, forwards all command-line arguments to rustc_driver
pub fn main() {
    let args: Vec<_> = env::args().collect();

    // configure debug logging...
    let mut logs = std::env::current_dir().unwrap();
    let logs = logs.into_boxed_path();
    let config = Arc::new(DatirConfig::debug(Some(logs)));

    let mut gather_info = callbacks::gather_orig::GatherAtiInfo::new(config.clone());
    rustc_driver::run_compiler(&args, &mut gather_info); // panics on compilation failure
    let fbs = gather_info.first_pass_info();

    // config to expose some optional functionality, for instance printing the 
    // instrumented source code, or outputing it to a file.
    let mut cbs = callbacks::transform_ast::TransformAbstractSyntaxTreeCallbacks::new(fbs, config);
    rustc_driver::run_compiler(&args, &mut cbs);
}
