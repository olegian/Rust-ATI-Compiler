/* This file defines the callbacks used by the second compilation, responsible
 * for actually modifying the AST to include instrumentation. Under the hood,
 * this file defines passes to run over the AST via the TransformingFileLoader,
 * so that every file being compiled gets properly instrumented, and not just the
 * crate root.
*/

use std::sync::Arc;

use rustc_ast::{ast, mut_visit::MutVisitor};
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;
use rustc_session::parse::ParseSess;

use crate::{
    common::DatirConfig,
    file_loaders::transforming_loader::{Passes, TransformingFileLoader},
    types::ati_info::FirstPassInfo,
    visitors::{TransformVisitor, add_crate_attribute, define_types_from_file, generate_stubs},
};

/// Callbacks used to transform the ASTs of all files being instrumented.
pub struct TransformAbstractSyntaxTreeCallbacks {
    first_pass: Arc<FirstPassInfo>,
    config: Arc<DatirConfig>,
}

impl TransformAbstractSyntaxTreeCallbacks {
    /// Constructor
    pub fn new(first_pass: FirstPassInfo, config: Arc<DatirConfig>) -> Self {
        Self {
            first_pass: Arc::new(first_pass),
            config,
        }
    }
}

impl rustc_driver::Callbacks for TransformAbstractSyntaxTreeCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        // use our custom loader to also instrument non-root files
        // this loader will be the one responsible for adding all stubs,
        // tupling all literals, etc.

        let first_pass = self.first_pass.clone();
        let datir_config = self.config.clone();
        let mut passes = Passes::new();
        passes.register(Box::new(
            move |psess: &ParseSess, mut krate: &mut ast::Crate, module_path: &str| {
                // Single visitor that performs both expression instrumentation
                // (literals, binary ops, calls, etc.) and type wrapping (Tagged<T>)
                // in one AST walk.
                let mut visitor =
                    TransformVisitor::new(&datir_config, &first_pass, psess, module_path);
                visitor.visit_crate(&mut krate);

                // create all required function stubs, which perform site management
                generate_stubs(&datir_config, &first_pass, krate, module_path, psess);
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
    /// Adds necessary compiler features.
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        let cwd = std::env::current_dir().unwrap();
        define_types_from_file(
            &cwd.join("src/ati/tagged_ops.rs"),
            &compiler.sess.psess,
            krate,
        );
        define_types_from_file(
            &cwd.join("src/ati/site_binds.rs"),
            &compiler.sess.psess,
            krate,
        );
        define_types_from_file(&cwd.join("src/ati/index.rs"), &compiler.sess.psess, krate);
        define_types_from_file(&cwd.join("src/ati/iterators.rs"), &compiler.sess.psess, krate);
        define_types_from_file(&cwd.join("src/ati/tagged.rs"), &compiler.sess.psess, krate);
        define_types_from_file(&cwd.join("src/ati/ati.rs"), &compiler.sess.psess, krate);
        add_crate_attribute(
            "#![feature(min_specialization)]",
            &compiler.sess.psess,
            krate,
        );
        add_crate_attribute("#![feature(step_trait)]", &compiler.sess.psess, krate);
        add_crate_attribute("#![feature(unsize)]", &compiler.sess.psess, krate);
        add_crate_attribute("#![feature(coerce_unsized)]", &compiler.sess.psess, krate);
        // For the --release main wrapper: produces a random filename for
        // the per-execution .ati output via std::random.
        add_crate_attribute("#![feature(random)]", &compiler.sess.psess, krate);

        Compilation::Continue
    }

    // leaving the other callbacks just in case they are useful
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        Compilation::Continue
    }
}
