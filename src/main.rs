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
#![feature(unsize)]
#![feature(coerce_unsized)]

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
extern crate smallvec;
extern crate thin_vec;

use std::{env, sync::Arc};

use crate::args::{ArgParser, ArgSpec};
use crate::common::DatirConfig;

use decls_gen::DeclsFile;

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
    let parser = arg_init(&program);
    let args = parser.parse_env();

    // Get path to main/lib.rs file being instrumented.
    let target_file = args
        .get_value("file")
        .expect("parser guarantees `file` is present")
        .to_string();
    let target_path = std::path::PathBuf::from(&target_file);

    // Generate / parse related .decls file.
    let decls_file = if let Some(path) = args.get_value("decls-path") {
        let decls_path = std::path::PathBuf::from(path);
        match DeclsFile::from_decls_file(&decls_path) {
            Ok(file) => file,
            Err(e) => panic!(
                "Unable to parse in decls file located at {decls_path:?}, failed with error: {e:?}"
            ),
        }
    } else {
        let depth = (!args.is_present("test")).then(|| {
            let d = args
                .get_value("rec-depth")
                .expect("Rec Depth did not have a value (even though it is default specified)");
            d.parse::<usize>()
                .expect("Unable to interpret rec-depth as an integer.")
        });

        DeclsFile::from_source_file(&target_path, depth)
    };

    // Construct config based on mode.
    let config = if let Some(dir_path) = args.get_value("release") {
        // remove stale files in output.
        let raw = std::path::PathBuf::from(dir_path);
        let _ = std::fs::remove_dir_all(&raw);
        std::fs::create_dir_all(&raw)
            .expect("Unable to create ATI output directory.");

        let output = std::fs::canonicalize(&raw)
            .expect("Unable to canonicalize ATI output directory.");
        DatirConfig::release(decls_file, output)
    } else if args.is_present("test") {
        DatirConfig::test(decls_file)
    } else {
        DatirConfig::debug(decls_file)
    };

    // Construct arguments to pass to rustc
    let mut compiler_args = vec![program, target_file];
    if let Some(output) = args.get_value("output") {
        compiler_args.push(format!("-o{output}"));
    }

    config.log("Config", format!("{:#?}", config));

    // The gather compilation
    // panics on compilation failure, therefore by the time the instrument
    // compilation starts, we know we are working with a semantically correct rust program
    let config = Arc::new(config);
    let mut gather_info = callbacks::gather_orig::GatherAtiInfo::new(config.clone());
    rustc_driver::run_compiler(&compiler_args, &mut gather_info);
    let first_pass = gather_info.into_first_pass_info();

    // The instrument compilation
    let mut cbs = callbacks::transform_ast::TransformAbstractSyntaxTreeCallbacks::new(
        first_pass,
        config.clone(),
    );
    rustc_driver::run_compiler(&compiler_args, &mut cbs);

    if !args.is_present("decls-path") {
        // output the decls file if we didnt parse one in.
        config
            .decls_file
            .write_to_file(&target_path.with_extension("decls"))
            .expect("unable to write decls file to disk");
    }
}

fn arg_init(program_name: &str) -> ArgParser {
    let parser = ArgParser::new(
        program_name,
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
        ArgSpec::keyword(
            "release",
            "Run in release mode, skipping debug logging, also creating .ati files\
             whenever the output binary is executed in the directory pointed to by ATI_OUT_DIR_PATH",
        )
        .long("--release")
        .short("-r")
        .value_name("ATI_OUT_DIR_PATH")
    )
    .arg(
        ArgSpec::keyword(
            "decls-path",
            "Rather than regenerating a decls file, parse in an existing one specified by PATH.",
        )
        .short("-d")
        .long("--decls-path")
        .value_name("PATH"),
    )
    .arg(
        ArgSpec::keyword(
            "rec-depth",
            "The recursive depth with which to expand all variables at each program point. \
             Defaults to 3. Only useful if --decls-path is left unspecified ",
        )
        .short("-rd")
        .long("--rec-depth")
        .value_name("INT_DEPTH")
        .default_value("3"),
    )
    .arg(ArgSpec::flag(
        "test",
        "--test",
        "Run in test mode, skipping debug logging, and using regular print ATI output",
    ));

    parser
}
