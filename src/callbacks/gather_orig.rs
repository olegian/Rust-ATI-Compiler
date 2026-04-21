/* Before we can perform the required AST mutation, we need to gather
 * some type information about the original source code. This is done by
 * invoking the compiler and passing in the GatherAtiInfo callback struct
 * defined in this file. See after_expansion below for more specific information
 * on what information is gathered.
*/
use rustc_ast as ast;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;
use std::sync::Arc;

use crate::{common::DatirConfig, types::ati_info::FirstPassInfo, visitors::AnalyzeHirVisitor};

/// Defines the callbacks used for the first information-gathering compilation.
pub struct GatherAtiInfo {
    /// Contains the information discovered after executing the compilation.
    first_pass: FirstPassInfo,
    config: Arc<DatirConfig>,
}

impl GatherAtiInfo {
    /// Constructor
    pub fn new(config: Arc<DatirConfig>) -> Self {
        Self {
            first_pass: Default::default(),
            config,
        }
    }

    /// Pulls out all gathered info that this compiler invocation learned.
    /// Must be called after the first compilation is performed.
    pub fn into_first_pass_info(self) -> FirstPassInfo {
        self.first_pass
    }
}

impl<'a> rustc_driver::Callbacks for GatherAtiInfo {
    /// Disables everything after MIR construction
    fn config(&mut self, config: &mut interface::Config) {
        config.opts.unstable_opts.no_codegen = true;
    }

    fn after_crate_root_parsing(
        &mut self,
        _compiler: &interface::Compiler,
        _krate: &mut ast::Crate,
    ) -> Compilation {
        Compilation::Continue
    }

    /// This is where the key functionality of this compiler invocation lies.
    /// Overall, the following is performed:
    ///   1. Find all locations (code spans) where:
    ///       a. 
    ///   2. Find all invocations of functions that are not defined in the instrumented files
    ///      (calls to code in libraries which was left uninstrumented).
    /// 
    /// As of 3/29/26, we are choosing to ignore uninstrumented libraries, meaning that
    /// (2) is really an unnecessary step. The goal is to instrument the standard library at least
    /// and after that is done, determine what needs to be added to this code to appropriately handle
    /// uninstrumented library code. The code is still left, as a proof-of-concept for later
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        // Iterates over all code blocks that can be invoked. This includes
        // regular functions, methods defined in impl blocks, closures, and
        // anon constants. All body owners receive a unique DefId and BodyId.
        // FIXME: This whole system needs a rework. Finding the "tracked boundary"
        // requires iterating through the entire crate (most likely file-by-file to be
        // able to differentiate where functions are defined), alongside namespace resolution
        // to differentiate TypeOne::foo from TypeTwo::foo. Is that all?
        for local_def_id in tcx.hir_body_owners() {
            let node = tcx.hir_node_by_def_id(local_def_id);

            if let rustc_hir::Node::Item(rustc_hir::Item {
                kind: rustc_hir::ItemKind::Fn { ident, .. },
                ..
            }) = node
            {
                // we found a regular function, named `ident`!
                self.first_pass
                    .observe_tracked_fn(&ident, local_def_id.to_def_id());
            } else if let rustc_hir::Node::ImplItem(rustc_hir::ImplItem {
                ident,
                kind: rustc_hir::ImplItemKind::Fn(_, _),
                ..
            }) = node
            {
                // we found a method defined in some impl block!
                self.first_pass
                    .observe_tracked_fn(ident, local_def_id.to_def_id());
            } else if let rustc_hir::Node::ImplItem(_) = node {
                // non-Fn impl items (associated constants, types)
                // FIXME: probably safe to ignore for now, but should be implemented soon
            } else if let rustc_hir::Node::AnonConst(_) = node {
                // static constants, like lengths of arrays that need to be computed
                // FIXME: probably safe to ignore for now, but should be implemented soon
            } else {
                // FIXME: implement support for closures. Should closures be treated as full blown functions?
                unimplemented!(
                    "Found body owner that isn't a function while discovering ATI info: {node:#?}"
                )
            }
        }

        // at this point, self.first_pass has knowledge of every single function that
        // requires instrumentation.

        let mut find_calls_visitor = AnalyzeHirVisitor {
            tcx,
            first_pass: &mut self.first_pass,
        };
        tcx.hir_walk_toplevel_module(&mut find_calls_visitor);

        // at this point, self.first_pass has knowledge of:
        // 1. every single function that requires instrumentation to be added
        // 2. all code locations where a funciton that is not instrumented is invoked
        if self.config.print_first_pass_info {
            self.config
                .log("FirstPassInfo", format!("{:#?}", self.first_pass));
        }

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
