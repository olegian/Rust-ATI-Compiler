/* This file defines a TransformingFileLoader. This struct can be used in place
 * of the regular file loader used by rustc to read in any file it is about to compile
 * by setting `config.file_loader` (easiest way to do that is by using the `config()`
 * callback). This custom loader allows for using the regular AST visitor pattern but to
 * mutate *any* file, and not just the root.
*/
use rustc_ast as ast;
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::common::{self, DatirConfig};

/// This FileLoader constructs an early intermediate AST of any file that is loaded
/// through it. This intermediate AST can be modified using the regular Visitor
/// pattern, before being written back into a string format and actually handed
/// off to the rest of rustc. Rustc will then reconstruct this same AST, which is
/// quite unfortunate. But, this overall allows for making AST modifications
/// to files which are not just the crate root using the same infrastructure as the
/// rest of the compiler.
pub struct TransformingFileLoader {
    /// The regular FileLoader that rustc uses
    inner: RealFileLoader,
    passes: Passes,
    config: Arc<DatirConfig>,
    /// Parent directory of the crate root file (main.rs / lib.rs).
    /// Discovered on the first Root file read and used to compute
    /// relative module paths for dep files.
    root_dir: Mutex<Option<PathBuf>>,
}

/// Represents the contents of a file being loaded, alongside metadata about
/// what kind of file it is and its Rust module path.
#[derive(Debug)]
pub struct FileContents {
    pub source: String,
    pub file_type: FileType,
    /// The Rust module path for this file, derived from the filesystem path.
    /// Empty string for the crate root (`main.rs` / `lib.rs`).
    /// For other files, segments are joined with `::` (e.g., `"dep"`, `"foo::bar"`).
    pub module_path: String,
}

impl FileContents {
    pub fn new(source: String, path: &Path, root_dir: Option<&Path>) -> Self {
        let path_str = path.to_str().unwrap_or("");

        // non .rs files, or std library files, external crates, etc.
        let (file_type, module_path) = if path.extension().and_then(|s| s.to_str()) != Some("rs")
            || path_str.contains("/.rustup/")
            || path_str.contains("/.cargo/")
            || path_str.contains("/rustc/")
        {
            (FileType::Untracked, String::new())
        } else if path_str.ends_with("main.rs") || path_str.ends_with("lib.rs") {
            // TODO: how do we know a file is the root based off the path?
            // as in, calling rustc <file> makes <file> the root, regardless of name.
            // do we need to use an ENV var or something? does rustc happen to set one?
            (FileType::Root, String::new())
        } else {
            (FileType::Dep, Self::derive_module_path(path, root_dir))
        };

        Self {
            source,
            file_type,
            module_path,
        }
    }

    /// Derives a Rust module path from a filesystem path, relative to the
    /// crate root directory.
    ///
    /// Given `root_dir = /project/tests/multi_file` and
    /// `path = /project/tests/multi_file/dep.rs`, produces `"dep"`.
    ///
    /// For nested paths like `/project/src/foo/bar.rs` with
    /// `root_dir = /project/src`, produces `"foo::bar"`.
    ///
    /// A trailing `mod` segment is dropped since `foo/mod.rs` represents
    /// the `foo` module, not `foo::mod`.
    ///
    /// Falls back to the file stem if the root dir is unknown or the path
    /// is not relative to it.
    fn derive_module_path(path: &Path, root_dir: Option<&Path>) -> String {
        let stem = path.with_extension("");

        let relative = if let Some(root) = root_dir {
            stem.strip_prefix(root).ok()
        } else {
            None
        };

        let segments: Vec<&str> = if let Some(rel) = relative {
            rel.components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect()
        } else {
            // Fallback: just use the file stem
            vec![
                stem.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown"),
            ]
        };

        let mut module_segments = segments;

        // `foo/mod.rs` -> module path is just `foo`, not `foo::mod`
        if module_segments.last() == Some(&"mod") {
            module_segments.pop();
        }

        module_segments.join("::")
    }
}

#[derive(Debug)]
pub enum FileType {
    /// Represents the tracked crate root file.
    Root,
    /// Represents a tracked dep file.
    Dep,
    /// Represents an untracked file.
    Untracked,
}

/// `module_path` is the Rust module path derived from the file being processed
/// (e.g., `""` for root, `"dep"` for `dep.rs`).
type Pass = Box<dyn Fn(&ParseSess, &mut ast::Crate, &FileType, &str) + Send + Sync>;
pub struct Passes(Vec<Pass>);
impl Passes {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn register(&mut self, pass: Pass) {
        self.0.push(pass);
    }

    fn iter(&self) -> impl Iterator<Item = &Pass> {
        self.0.iter()
    }
}

impl TransformingFileLoader {
    /// Constructor
    pub fn new(passes: Passes, config: Arc<DatirConfig>) -> Self {
        Self {
            inner: RealFileLoader,
            passes,
            config,
            root_dir: Mutex::new(None),
        }
    }

    /// Creates a new parser
    fn create_parse_sess() -> ParseSess {
        ParseSess::new(Vec::from([rustc_driver::DEFAULT_LOCALE_RESOURCE]))
    }

    fn transform_source(&self, file: FileContents, path: &Path) -> String {
        let psess = Self::create_parse_sess();

        let mut krate = common::parse_crate(&psess, file.source, Some(path));

        for pass in self.passes.iter() {
            pass(&psess, &mut krate, &file.file_type, &file.module_path);
        }

        let output = self.ast_to_source(&krate);

        if self.config.print_transformed_source {
            self.config.log(
                "TransformedSource",
                format!("======== {path:?} ========\n{output}\n"),
            );
        }

        output
    }

    /// Converts a Crate AST to a standard string representation, equivalent
    /// to that of a regular source file. After this call, the regular rustc
    /// parser will be ready to run again consuming the output string.
    fn ast_to_source(&self, krate: &ast::Crate) -> String {
        let mut output = String::new();

        // probably unnecessary right now, but these are the only "other thing"
        // in the krate that is found in the source file.
        for attr in &krate.attrs {
            let attr_str = pprust::attribute_to_string(attr);
            output.push_str(&attr_str);
            output.push('\n');
        }

        for item in &krate.items {
            let item_str = pprust::item_to_string(&item);
            output.push_str(&item_str);
            output.push_str("\n\n"); // two \n just to match normal file loader
        }

        output
    }

    /// Reads in a file at `path` directly, constructing a FileContents
    /// which determines the file type and module path from the path.
    /// When a Root file is encountered, its parent directory is saved
    /// for use in computing relative module paths for dep files.
    fn read_file(&self, path: &Path) -> io::Result<FileContents> {
        let source = self.inner.read_file(path)?;

        let root_dir_guard = self.root_dir.lock().unwrap();
        let root_dir_ref = root_dir_guard.as_deref();
        let file = FileContents::new(source, path, root_dir_ref);
        drop(root_dir_guard);

        // If this is the root file, remember its parent directory
        if matches!(file.file_type, FileType::Root) {
            if let Some(parent) = path.parent() {
                *self.root_dir.lock().unwrap() = Some(parent.to_path_buf());
            }
        }

        Ok(file)
    }
}

/// Implements the necessary trait to use the custom loader as a
/// file loader in the compiler.
impl FileLoader for TransformingFileLoader {
    /// Returns true if the file at `path` exists
    fn file_exists(&self, path: &Path) -> bool {
        self.inner.file_exists(path)
    }

    /// Reads the file point to by `path` into a String. This function
    /// will actually do the transformations defined in the TransformingFileLoader.
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let file_contents = self.read_file(path)?;

        // If we ever read in a file that we are not instrumenting,
        // then just pass the contents up, skipping the transformation step.
        if matches!(file_contents.file_type, FileType::Untracked) {
            Ok(file_contents.source)
        } else {
            Ok(self.transform_source(file_contents, path))
        }
    }

    // Would we ever do this? I guess if we do like extern linking? idk when this is invoked.
    fn read_binary_file(&self, path: &Path) -> io::Result<std::sync::Arc<[u8]>> {
        unimplemented!()
    }

    /// Gets the current directory.
    fn current_directory(&self) -> io::Result<std::path::PathBuf> {
        std::env::current_dir()
    }
}
