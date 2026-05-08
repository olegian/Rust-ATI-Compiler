//! Defines a rustc-compatible file loader, which uniformly applies a set of AST transformations
//! to every file being compiled.
//!
//! To utilize the [`TransformingFileLoader`], construct it with a collection of [`Passes`]
//! which operate over the AST of each loaded file, then assign it to `config.file_loader` within
//! the `config` callback. See [crate::callbacks::instrument] for an example.
//!
//! The `after_crate_root_parsing` callback is the only one provided by rustc within which
//! AST modification is possible. This callback provides a mutable reference to the
//! `rustc_ast::Crate`, however, this crate only captures the crate root, in other words, `main.rs`
//! or `lib.rs`. Therefore, any modification defined there will only be represented in that file.
//! The later callbacks, `after_expansion` and `after_analysis` get invoked when the AST has
//! already been lowered to an HIR, which makes the crate immutable.
//!
//! The [`TransformingFileLoader`] avoids this problem, allowing for AST-level modification of all
//! compiled files, by constructing a "preliminary AST" before handing off any file rustc.
//! This means the usual compilation pipeline of:
//! ```text
//! 1. File Loading
//! 2. Source to AST
//! 3. AST to HIR
//! 4. HIR to THIR
//! 5. ...
//! ```
//! instead becomes:
//! ```text
//! 1. File Loading --------------------
//! 2. Source to AST           |  done by [`TransformingFileLoader`]
//! 3. AST Transformation      |
//! 4. AST to Source String -----------
//! 5. Source to AST           |  done by rustc
//! 6. AST to HIR              |
//! 7. HIR to THIR             |
//! 8. ...
//! ```
//!
//! This incurs a slight runtime cost in requiring the pipeline to construct another AST, but allows
//! for instrumentation of non-root files utilizing standard rustc `Visitor`/`MutVisitor` patterns.

mod files;
mod transforming_passes;

use crate::callbacks::parsing;
use crate::config::DatirConfig;
use files::{FileContents, FileType};
pub use transforming_passes::Passes;

/// File loader responsible for loading files from disk and applying a transformation to them.
/// 
/// This FileLoader constructs an early intermediate AST of any file that is loaded
/// through it. This intermediate AST can be modified using the regular Visitor
/// pattern, before being written back into a string format and actually handed
/// off to the rest of rustc.
pub struct TransformingFileLoader {
    /// The regular FileLoader that rustc uses
    inner: rustc_span::source_map::RealFileLoader,
    /// A list of closures that operate over the intermediate AST to transform it.
    passes: Passes,
    /// DATIR configuration which governs the entire instrumentation process.
    config: std::sync::Arc<DatirConfig>,
    /// Parent directory of the crate root file (`main.rs` / `lib.rs`).
    /// Discovered on the first Root file read and used to compute
    /// relative module paths for dep files.
    root_dir: std::sync::OnceLock<std::path::PathBuf>,
}

/// Implements the necessary trait to use the custom loader as a
/// file loader in the compiler.
impl rustc_span::source_map::FileLoader for TransformingFileLoader {
    /// Returns true if the file at `path` exists
    fn file_exists(&self, path: &std::path::Path) -> bool {
        self.inner.file_exists(path)
    }

    /// Reads the file point to by `path` into a String. This function
    /// will actually do the transformations defined in the TransformingFileLoader.
    fn read_file(&self, path: &std::path::Path) -> std::io::Result<String> {
        let file_contents = self.load_file_contents(path)?;

        // If we ever read in a file that we are not instrumenting,
        // then just pass the contents up, skipping the transformation step.
        if matches!(file_contents.file_type, FileType::Untracked) {
            Ok(file_contents.source)
        } else {
            Ok(self.transform_source(file_contents, path))
        }
    }

    // Would we ever do this? maybe for extern linking?
    fn read_binary_file(&self, _path: &std::path::Path) -> std::io::Result<std::sync::Arc<[u8]>> {
        unimplemented!()
    }

    /// Gets the current directory.
    fn current_directory(&self) -> std::io::Result<std::path::PathBuf> {
        std::env::current_dir()
    }
}

impl TransformingFileLoader {
    /// Constructor
    pub fn new(passes: Passes, config: std::sync::Arc<DatirConfig>) -> Self {
        Self {
            inner: rustc_span::source_map::RealFileLoader,
            passes,
            config,
            root_dir: std::sync::OnceLock::new(),
        }
    }

    /// Runs all registered `Passes` over a loaded file.
    /// 
    /// Given a loaded source file (represented as a string, within `file`), parses it into
    /// an AST, executes each of the AST-transforming passes over it, then converts the
    /// modified AST back into a source string representation.
    fn transform_source(&self, file: FileContents, path: &std::path::Path) -> String {
        let psess = rustc_session::parse::ParseSess::new();
        let mut krate = parsing::parse_crate(&psess, file.source, Some(path));
        if self.config.print_original_ast {
            self.config.log(
                "OriginalAst",
                format!("======== {path:?} ========\n{krate:#?}\n"),
            );
        }

        for pass in self.passes.iter() {
            pass(&psess, &mut krate, &file.module_path);
        }

        let output = ast_to_source(&krate);

        if self.config.print_transformed_ast {
            self.config.log(
                "TransformedAst",
                format!("======== {path:?} ========\n{krate:#?}\n"),
            );
            self.config.log(
                "TransformedSource",
                format!("======== {path:?} ========\n{output}\n"),
            );
        }

        output
    }

    /// Reads in a file at `path` directly, constructing a `FileContents`.
    /// 
    /// This struct determines the file type and module path from the file path.
    /// When a Root file is encountered, its parent directory is saved
    /// for use in computing relative module paths for dependancy files.
    fn load_file_contents(&self, path: &std::path::Path) -> std::io::Result<FileContents> {
        let source = rustc_span::source_map::FileLoader::read_file(&self.inner, path)?;
        let file = FileContents::new(source, path, self.root_dir.get().map(|p| p.as_path()));

        // If this is the root file, remember its parent directory
        if matches!(file.file_type, FileType::Root) {
            if let Some(parent) = path.parent() {
                let _ = self.root_dir.set(parent.to_path_buf());
            }
        }

        Ok(file)
    }
}

/// Converts a Crate AST to a standard source string representation.
/// 
/// The output string is equivalent to that of a regular source file. After this call, 
/// the regular rustc parser will be ready to run again, to consume this output string.
fn ast_to_source(krate: &rustc_ast::Crate) -> String {
    let mut output = String::new();

    // probably unnecessary right now, but these are the only "other thing"
    // in the krate that is found in the source file.
    for attr in &krate.attrs {
        let attr_str = rustc_ast_pretty::pprust::attribute_to_string(attr);
        output.push_str(&attr_str);
        output.push('\n');
    }

    for item in &krate.items {
        let item_str = rustc_ast_pretty::pprust::item_to_string(&item);
        output.push_str(&item_str);
        output.push_str("\n\n"); // two \n just to match normal file loader
    }

    output
}
