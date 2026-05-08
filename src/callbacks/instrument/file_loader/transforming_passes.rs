//! Provides a type-safe way of specifying AST transformations performed by
//! the [super::TransformingFileLoader].
//!
//! The [super::TransformingFileLoader] constructs a preliminary AST of every instrumented file
//! before passing it off to the rustc compilation pipeline. This preliminary AST
//! can have multiple "passes" defined over it, ultimately closures which modify the AST in place.
//! This is done by accepting a mutable reference to the loaded crate as an input to each
//! closure.
//!
//! For modularities sake, it's possible to define multiple closures which get run over the AST
//! in sequence, or a single closure can capture all required transformations.

/// The type signature for a single closure which operates over an AST loaded by 
/// the [super::TransformingFileLoader].
/// 
/// `module_path` is the Rust module path derived from the file being processed
/// (e.g., `""` for root, `"dep"` for `dep.rs`). View 
/// [crate::callbacks::instrument::file_loader::files] for more
/// information.
type Pass =
    Box<dyn Fn(&rustc_session::parse::ParseSess, &mut rustc_ast::Crate, &str) + Send + Sync>;

/// Wrapper type over a series of [`Pass`]es.
pub struct Passes(Vec<Pass>);
impl Passes {
    /// Construct a new set of passes.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    // Add a pass to the set.
    pub fn register(&mut self, pass: Pass) {
        self.0.push(pass);
    }

    // Iterate over all passes within the set.
    pub fn iter(&self) -> impl Iterator<Item = &Pass> {
        self.0.iter()
    }
}
