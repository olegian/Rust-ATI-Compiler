use rustc_ast::ast;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

use crate::file_loaders::transforming_loader::{
    FileType, Passes, TransformingFileLoader, TransformingFileLoaderConfig
};

pub struct Explicit {}

impl Explicit {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'a> rustc_driver::Callbacks for Explicit {
    fn config(&mut self, config: &mut interface::Config) {
        // use our custom loader to also instrument non-root files
        // this loader will be the one responsible for adding all stubs,
        // tupling all literals, etc.
        let mut passes = Passes::new();
        passes.register(Box::new(|krate: &mut ast::Crate, ftype: &FileType| {

        }));

        passes.register(Box::new(|krate: &mut ast::Crate, ftype: &FileType| {

        }));

        config.file_loader = Some(Box::new(TransformingFileLoader::new(
            passes,
            TransformingFileLoaderConfig::debug(),
        )));
    }

    /// Define necessary types in the root file. All other files will
    /// import these types from the root.
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
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
