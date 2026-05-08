//! DATIR entry points.
//!
//! This file parses input command line arguments, then orchestrates the multiple compiler 
//! invocations required to perform abstract type inference, producing an instrumented binary 
//! that outputs comparability information regarding the parameters and returned values from each
//! function (and a `.decls` file, describing the declared program points and variables).
//!
//! The first compilation gathers necessary information about the source code
//! (namely some type information), which the second compilation uses to actually mutate the
//! AST and to add in dynamic instrumentation.
//! 
//! See [callbacks] for information about the two compilation steps.
//!
//! See --help for usage instructions.

// DATIR utilizes the following unstable compiler features:
// provides access to rustc internal functions
#![feature(rustc_private)]
// allows using the `box` keyword in patterns to match on std::Box
#![feature(box_patterns)]

// allows performing trait specialization, which the runtime ati library
// makes heavy use of to dispatch the appropriate function during monomorphization.
// In other words, if we define some trait MyTrait, and then implement:
// > impl<T>    MyTrait for T          { fn foo() ... }
// > impl<T>    MyTrait for &T         { fn foo() ... }
// > impl<T, N> MyTrait for [T; N]     { fn foo() ... }
// > impl<T>    MyTrait for Wrapper<T> { fn foo() ... }
// we can always call T.foo(), regardless of what T is, as there is a
// default implementation for all generic Ts. However, if T.foo() is invoked,
// and T is actually a Wrapper<T>, then it will dispatch the foo() defined
// for Wrapper<T> rather than for T. Without min_specialization, those trait
// implementations would overlap.
// Note: full_specialization is unsound, and also unnecessary here.
// Another note: This feature is only necessary if `mod ati` is uncommented below.
#![feature(min_specialization)]

#![feature(step_trait)]

// allows defining a Dynamically Sized Type (used for representing Tagged References
// to Unsized types like [T]) while allowing automatic coercion from a Sized type.
// Note: These features are only necessary if `mod ati` is uncommented below.
#![feature(unsize)]
#![feature(coerce_unsized)]

// All linked in rustc_private crates
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

use crate::config::DatirConfig;
use decls_gen::DeclsFile;

// include so VsCode's rust-analyzer extension runs static analysis on the runtime library
mod ati;

mod args;
mod callbacks;
mod config;

/// Errors produced by [`run`].
#[derive(Debug)]
pub enum DatirError {
    BadInput(&'static str),
}

impl std::fmt::Display for DatirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatirError::BadInput(msg) => write!(f, "{msg}"),
        }
    }
}

/// Executes DATIR end-to-end: runs both compiler passes against `target`,
/// producing an instrumented binary at `output` (or rustc's default location
/// if `None`). Use this entrypoint to invoke DATIR programmatically.
fn run(
    config: DatirConfig,
    target: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<(), DatirError> {
    let mut rustc_args = vec![
        "datir".to_string(),
        target
            .to_str()
            .ok_or(DatirError::BadInput(
                "Unable to parse input target path as UTF-8 string.",
            ))?
            .to_string(),
    ];

    if let Some(output) = output {
        let output = output.to_str().ok_or(DatirError::BadInput(
            "Unable to parse output path as UTF-8 string.",
        ))?;
        rustc_args.push(format!("-o{output}"));
    }

    if config.print_config {
        config.log("Config", format!("{:#?}", config));
    }

    // The gather compilation
    // panics on compilation failure, therefore by the time the instrument
    // compilation starts, we know we are working with a semantically correct rust program
    let config = std::sync::Arc::new(config);
    let mut gather_info = callbacks::gather::GatherAtiInfo::new(config.clone());
    rustc_driver::run_compiler(&rustc_args, &mut gather_info);
    let first_pass = gather_info.into_first_pass_info();

    // The instrument compilation
    let mut cbs = callbacks::instrument::TransformAbstractSyntaxTreeCallbacks::new(
        first_pass,
        config.clone(),
    );
    rustc_driver::run_compiler(&rustc_args, &mut cbs);

    Ok(())
}

/// Parses DATIR's command-line options into the
/// inputs that [`run`] expects, then delegates.
fn main() {
    // this is mostly a placeholder string, for printing a nice usage message.
    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "datir".to_string());
    let args = args::datir_arg_init(&program).parse_env();

    // Get path to main/lib.rs file being instrumented.
    let target_path = std::path::PathBuf::from(
        args.get_value("file")
            .expect("parser guarantees `file` is present"),
    );

    // Generate / parse related .decls file. When generating fresh, also
    // write it to disk, so subsequent runs can reuse it via
    // --decls-path.
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
            args.get_value("rec-depth")
                .expect("Rec Depth did not have a value (even though it is default specified)")
                .parse::<usize>()
                .expect("Unable to interpret rec-depth as an integer.")
        });

        let decls_file = DeclsFile::from_source_file(&target_path, depth);
        decls_file
            .write_to_file(&target_path.with_extension("decls"))
            .expect("unable to write decls file to disk");
        decls_file
    };

    // Construct config based on mode.
    let config = if let Some(dir_path) = args.get_value("release") {
        let raw = std::path::PathBuf::from(dir_path);
        let _ = std::fs::remove_dir_all(&raw);
        std::fs::create_dir_all(&raw).expect("Unable to create ATI output directory.");
        let output_dir =
            std::fs::canonicalize(&raw).expect("Unable to canonicalize ATI output directory.");
        DatirConfig::release(decls_file, output_dir)
    } else if args.is_present("test") {
        DatirConfig::test(decls_file)
    } else {
        DatirConfig::debug(decls_file)
    };

    let output_path = args.get_value("output").map(std::path::PathBuf::from);

    if let Err(e) = run(config, &target_path, output_path.as_deref()) {
        eprintln!("datir: {e}");
        std::process::exit(1);
    }
}
