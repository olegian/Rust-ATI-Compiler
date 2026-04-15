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
#![feature(step_trait)]
#![feature(new_range_api)]

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

use crate::args::{ArgParser, ArgSpec};
use crate::common::DatirConfig;

// included so VsCode's rust-analyzer extension runs static analysis on the runtime library
mod ati;

mod args;
mod callbacks;
mod common;
mod file_loaders;
mod types;
mod visitors;

/// Entry-point. Parses DATIR's own command-line options, then forwards just
/// the source file path to each rustc compiler invocation.
pub fn main() {
    let program = env::args().next().unwrap_or_else(|| "datir".to_string());
    let parser = ArgParser::new(
        program.clone(),
        "DATIR: dynamic abstract type inference for Rust",
    )
    .arg(ArgSpec::positional(
        "file",
        "FILE",
        "Path to root source file to instrument",
    ))
    .arg(
        ArgSpec::keyword(
            "output",
            "Location of produced executable with added instrumentation",
        )
        .short("-o")
        .long("--output")
        .value_name("PATH"),
    )
    .arg(
        ArgSpec::flag(
            "release",
            "--release",
            "Run in release mode, skipping debug logging, and creating .decls file",
        )
        .short("-r"),
    )
    .arg(
        ArgSpec::flag(
            "test",
            "--test",
            "Run in test mode, skipping debug logging, and using regular print ATI output",
        )
        .short("-r"),
    );

    let args = parser.parse_env();


    let file_path = args
        .get_value("file")
        .expect("parser guarantees `file` is present")
        .to_string();


    let config = if args.is_present("release") {
        let output = std::path::Path::new(&file_path).with_extension("decls");
        Arc::new(DatirConfig::release(output))
    } else if args.is_present("test") {
        Arc::new(DatirConfig::test())
    } else {
        Arc::new(DatirConfig::debug())
    };

    let mut compiler_args = vec![program, file_path];
    if let Some(output) = args.get_value("output") {
        compiler_args.push(format!("-o{output}"))
    }

    let mut gather_info = callbacks::gather_orig::GatherAtiInfo::new(config.clone());
    rustc_driver::run_compiler(&compiler_args, &mut gather_info); // panics on compilation failure
    let first_pass = gather_info.into_first_pass_info();

    let mut cbs =
        callbacks::transform_ast::TransformAbstractSyntaxTreeCallbacks::new(first_pass, config);
    rustc_driver::run_compiler(&compiler_args, &mut cbs);
}
