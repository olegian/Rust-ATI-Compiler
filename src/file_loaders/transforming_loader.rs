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
use std::path::Path;

use crate::common;

/// This FileLoader constructs an early intermediate AST of any file that is loaded
/// through it. This intermediate AST can be modified using the regular Visitor
/// pattern, before being written back into a string format and actually handed
/// off to the rest of rustc. Rustc will then reconstruct this same AST, which is
/// quite unfortunate. But, this overall allows for making AST modifications
/// to files which are not just the crate root using the same infrastructure as the
/// rest of the compiler.
// NIT: would be nice to make this accept a list of visitors to execute
pub struct TransformingFileLoader {
    /// The regular FileLoader that rustc uses
    inner: RealFileLoader,
    passes: Passes,
    config: TransformingFileLoaderConfig,
}

/// Represents the string contents of a file, alongside the type of file
type FileContents = (String, FileType);
#[derive(Debug)]
pub enum FileType {
    /// Represents the tracked crate root file.
    Root,
    /// Represents a tracked dep file.
    Dep,
    /// Represents an untracked file.
    Untracked,
}

pub struct TransformingFileLoaderConfig {
    print_output: bool,
}

impl TransformingFileLoaderConfig {
    pub fn debug() -> Self {
        Self { print_output: true }
    }

    pub fn release() -> Self {
        Self { print_output: false }
    }
}

type Pass = Box<dyn Fn(&ParseSess, &mut ast::Crate, &FileType) + Send + Sync>;
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
    pub fn new(
        passes: Passes,
        config: TransformingFileLoaderConfig,
    ) -> Self {
        Self {
            inner: RealFileLoader,
            passes,
            config,
        }
    }

    /// Creates a new parser
    fn create_parse_sess() -> ParseSess {
        ParseSess::new(Vec::from([rustc_driver::DEFAULT_LOCALE_RESOURCE]))
    }

    fn transform_source(&self, contents: FileContents, path: &Path) -> String {
        let psess = Self::create_parse_sess();
        let (contents, file_type) = contents;

        let mut krate = common::parse_crate(&psess, contents, Some(path));

        for pass in self.passes.iter() {
            pass(&psess, &mut krate, &file_type);
        }

        let output = self.ast_to_source(&krate);

        if self.config.print_output {
            println!("===========  Modified File: {path:?} =============\n{output}")
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

    /// Reads in a file at `path` directly, while also determining what kind of
    /// file it is (Root, Dep, or Untracked)
    fn read_file(&self, path: &Path) -> io::Result<FileContents> {
        let contents = self.inner.read_file(path)?;
        let path_str = path.to_str().unwrap_or("");

        // non .rs files, or std library files, external crates, etc.
        let file_type = if path.extension().and_then(|s| s.to_str()) != Some("rs")
            || path_str.contains("/.rustup/")
            || path_str.contains("/.cargo/")
            || path_str.contains("/rustc/")
        {
            FileType::Untracked
        } else if path_str.ends_with("main.rs") || path_str.ends_with("lib.rs") {
            // TODO: how do we know a file is the root based off the path?
            // as in, calling rustc <file> makes <file> the root, regardless of name.
            // do we need to use an ENV var or something? does rustc happen to set one?
            FileType::Root
        } else {
            FileType::Dep
        };

        Ok((contents, file_type))
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
        if matches!(file_contents.1, FileType::Untracked) {
            Ok(file_contents.0)
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
