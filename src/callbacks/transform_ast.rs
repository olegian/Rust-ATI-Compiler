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
    file_loaders::transforming_loader::{FileType, Passes, TransformingFileLoader},
    types::ati_info::FirstPassInfo,
    visitors::{
        TransformVisitor, add_crate_attribute, define_types_from_file, generate_stubs, import_root_crate
    },
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

impl<'a> rustc_driver::Callbacks for TransformAbstractSyntaxTreeCallbacks {
    fn config(&mut self, config: &mut interface::Config) {
        // use our custom loader to also instrument non-root files
        // this loader will be the one responsible for adding all stubs,
        // tupling all literals, etc.

        let first_pass = self.first_pass.clone();
        let datir_config = self.config.clone();
        let mut passes = Passes::new();
        passes.register(Box::new(
            move |psess: &ParseSess,
                  mut krate: &mut ast::Crate,
                  ftype: &FileType,
                  module_path: &str| {
                // Single visitor that performs both expression instrumentation
                // (literals, binary ops, calls, etc.) and type wrapping (Tagged<T>)
                // in one AST walk.
                let mut visitor = TransformVisitor::new(&first_pass, psess);
                visitor.visit_crate(&mut krate);

                // create all required function stubs, which perform site management
                generate_stubs(&datir_config, krate, &first_pass, module_path, psess);

                // make the ATI types available to dependancies
                if matches!(ftype, FileType::Dep) {
                    import_root_crate(&mut krate, &psess);
                }
            },
        ));

        // use custom file loader to run passes over AST before continuing compilation
        config.file_loader = Some(Box::new(TransformingFileLoader::new(
            passes,
            self.config.clone(),
        )));
    }

    /// Define necessary types in the root file. All other files will
    /// import these types from the root.
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        let cwd = std::env::current_dir().unwrap();
        define_types_from_file(&cwd.join("src/ati/ati.rs"), &compiler.sess.psess, krate);
        define_types_from_file(&cwd.join("src/ati/tagged.rs"), &compiler.sess.psess, krate);
        add_crate_attribute(
            "#![feature(min_specialization)]",
            &compiler.sess.psess,
            krate,
        );

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
