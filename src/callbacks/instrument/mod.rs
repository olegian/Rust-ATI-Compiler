//! Defines the callbacks used by the second compilation, the "Instrument" pass.
//!
//! Under the hood, this file defines passes to run over the AST via the [`TransformingFileLoader`]
//! so that every file being compiled gets properly instrumented, and not just the
//! crate root.
//!
//! The overall transformation can be split into five steps:
//! 1. Modify the existing AST to tuple all values, modify assignments, update type hints, etc,
//!    such that value interactions are properly recorded.
//! 2. Define new function shims to perform site management, associating formals, return values,
//!    and globals with each ENTER and EXIT program point declared with the .decls file.
//! 3. Generate necessary trait implementations for user-defined compound types.
//! 4. Inject the runtime library into the crate root file, and import the root file in all
//!    dependancy files to make all ATI-specific types and globals available everywhere within the
//!    crate.
//! 5. Inject into the crate root file feature attributes to make necessary unstable features
//!    available for use.
//!
//! Steps 1-3 happen within the [`TransformingFileLoader`] constructed within the below `config`
//! callback. Step 4 is split between the code generation step within the file loader (for all root
//! imports within dependancies), and `after_crate_root_parsing` (to inject the runtime library)
//! into the main file. Step 5 also takes place in `after_crate_root_parsing`.

mod expr;
mod hoisting;
mod instrument;
mod item;
mod types;

use crate::{
    callbacks::codegen::{self, define_types},
    callbacks::gather::first_pass_info::FirstPassInfo,
    callbacks::instrument::instrument::InstrumentingVisitor,
    config::DatirConfig,
    file_loader::{Passes, TransformingFileLoader},
};

/// Crate-level attributes that must be injected into the root file to enable
/// the unstable features used by the generated code.
const REQUIRED_CRATE_ATTRIBUTES: &[&str] = &[
    "#![feature(min_specialization)]",
    "#![feature(step_trait)]",
    "#![feature(unsize)]",
    "#![feature(coerce_unsized)]",
    "#![feature(random)]", // only used when --release is specified
];

/// Callback struct used to transform the ASTs of all instrumented files.
pub struct TransformAbstractSyntaxTreeCallbacks {
    first_pass: std::sync::Arc<FirstPassInfo>,
    config: std::sync::Arc<DatirConfig>,
}

impl TransformAbstractSyntaxTreeCallbacks {
    /// Constructor
    pub fn new(first_pass: FirstPassInfo, config: std::sync::Arc<DatirConfig>) -> Self {
        Self {
            first_pass: std::sync::Arc::new(first_pass),
            config,
        }
    }
}

impl rustc_driver::Callbacks for TransformAbstractSyntaxTreeCallbacks {
    /// Define the transformations performed by the custom file loader,
    /// and then register this compiler invocation to use it instead of the
    /// default one.
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        let first_pass = self.first_pass.clone();
        let datir_config = self.config.clone();

        let mut passes = Passes::new();
        passes.register(Box::new(
            move |psess: &rustc_session::parse::ParseSess,
                  mut krate: &mut rustc_ast::Crate,
                  module_path: &str| {
                // Single visitor that performs both expression instrumentation
                // (literals, binary ops, calls, etc.) and type wrapping (Tagged<T>)
                // in one AST walk.
                let mut visitor =
                    InstrumentingVisitor::new(psess, &datir_config, &first_pass, module_path);
                rustc_ast::mut_visit::MutVisitor::visit_crate(&mut visitor, &mut krate);

                // create all required function stubs, which perform site management
                codegen::generate_shims(&datir_config, &first_pass, krate, module_path, psess);
            },
        ));

        // use custom file loader to run passes over AST before continuing compilation
        config.file_loader = Some(Box::new(TransformingFileLoader::new(
            passes,
            self.config.clone(),
        )));
    }

    /// Defines necessary types (namely Tagged<T>, but also globals like ATI_ANALYSIS)
    /// in the root file. All other files will import these types from the root.
    /// Further enables all necessary unstable features.
    fn after_crate_root_parsing(
        &mut self,
        compiler: &rustc_interface::interface::Compiler,
        krate: &mut rustc_ast::Crate,
    ) -> rustc_driver::Compilation {
        let cwd = std::env::current_dir().unwrap();
        inject_ati_directory(&cwd.join("src/ati"), &compiler.sess.psess, krate);
        inject_crate_attributes(&compiler.sess.psess, krate);

        rustc_driver::Compilation::Continue
    }

    // leaving the other callbacks just in case they are useful
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        _tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        rustc_driver::Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        _tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        rustc_driver::Compilation::Continue
    }
}

/// Injects every `.rs` file in `dir` (except `mod.rs`) into `krate` via
/// [`define_types::define_types_from_file`].
fn inject_ati_directory(
    dir: &std::path::Path,
    psess: &rustc_session::parse::ParseSess,
    krate: &mut rustc_ast::Crate,
) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read ati directory {dir:?}: {e}"));

    for entry in entries {
        let path = entry.unwrap().path();
        // skip non .rs files
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        // skip the mod.rs file
        if path.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
            continue;
        }
        define_types::define_types_from_file(&path, psess, krate);
    }
}

/// Injects every attribute in [`REQUIRED_CRATE_ATTRIBUTES`] into `krate`.
fn inject_crate_attributes(psess: &rustc_session::parse::ParseSess, krate: &mut rustc_ast::Crate) {
    for attr in REQUIRED_CRATE_ATTRIBUTES {
        define_types::add_crate_attribute(attr, psess, krate);
    }
}
