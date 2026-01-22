#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;

use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

use std::env;

mod instrumentation;
use crate::instrumentation::{
    TupleLiteralsVisitor, UpdateFnDeclsVisitor, create_stubs, define_types_from_file,
};

// included just for code analysis to run on ati.rs
mod ati;

// TODO: none of this code right now handles anything but pure functions.
// idk what to do with closures, and then associated functions need
// extra handling / visiting logic as well.

struct Callbacks {}
impl rustc_driver::Callbacks for Callbacks {
    /// Called before creating the compiler instance
    fn config(&mut self, _config: &mut interface::Config) {}

    /// Called after parsing the crate root. Submodules are not yet parsed when
    /// this callback is called. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        // discovers all functions that will be instrumented, and updates
        // the function signatures to tag all passed values as necessary
        let mut modify_params_visitor = UpdateFnDeclsVisitor::new();
        modify_params_visitor.visit_crate(krate);
        let modified_funcs = modify_params_visitor.get_modified_funcs();

        // tuple all literals to create tags, untupling as necessary
        // when they are passed into untracked functions
        let mut visitor = TupleLiteralsVisitor::new(modified_funcs);
        visitor.visit_crate(krate);

        // create all required function stubs, which perform site management
        create_stubs(krate, &compiler.sess.psess, modified_funcs);

        // define all used ATI types from ati.rs
        let cwd = std::env::current_dir().unwrap();
        define_types_from_file(
            &cwd.join("src/ati/ati.rs"),
            &compiler.sess.psess,
            krate,
        );

        Compilation::Continue
    }

    /// Called after expansion. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        Compilation::Continue
    }

    /// Called after analysis. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        Compilation::Continue
    }
}

/// Entry-point, forwards all arguments to rustc
pub fn main() {
    let args: Vec<_> = env::args().collect();
    let mut cbs = Callbacks {};
    rustc_driver::run_compiler(&args, &mut cbs);
}
