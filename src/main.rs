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

use std::{env, path::Path};

mod instrumentation;
use crate::instrumentation::{
    ATIVisitor, ModifyParamsVisitor, UpdateInvocationsVisitor, define_types_from_file,
};

// TODO: none of this code right now handles anything but pure functions.
//       idk what to do with closures, and then associated functions need
//       extra handling / visiting logic as well.

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
        // double formals for tags, and also pass around ATI struct between functions
        let mut modify_params_visitor = ModifyParamsVisitor::new(&compiler.sess.psess);
        modify_params_visitor.visit_crate(krate);

        // make sure all invocations of modified functions pass in appropriate values
        let modified_funcs = modify_params_visitor.get_modified_funcs();
        let mut update_invocations_visitor = UpdateInvocationsVisitor::new(modified_funcs);
        update_invocations_visitor.visit_crate(krate);

        // mutate each function body to add preludes, epilogues, and unifications
        let mut visitor = ATIVisitor::new(&compiler.sess.psess);
        visitor.visit_crate(krate);

        // add required ATI types to crate
        define_types_from_file(
            // TODO: reference this file in a better way
            Path::new("/home/olegian/TRACTOR/queries/src/ati/ati.rs"),
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

fn main() {
    let args: Vec<_> = env::args().collect();
    let mut cbs = Callbacks {};
    rustc_driver::run_compiler(&args, &mut cbs);
}
