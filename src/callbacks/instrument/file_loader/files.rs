//! Determines information regarding how to transform a file loaded by
//! the [super::TransformingFileLoader].
//!
//! Every file used within a crate is going to be loaded by the transforming
//! file loader, but not all files require instrumentation. [`FileContents`] captures the
//! contents of a loaded file, along with information regarding what file was loaded and
//! whether or not it requires the transformation to be applied to it.

/// Represents the contents of a file being loaded, alongside metadata about
/// what kind of file it is and the file's Rust module path.
#[derive(Debug)]
pub struct FileContents {
    /// The contents of the file.
    pub source: String,
    /// The type of file, with respect to whether or not it requires instrumentation.
    pub file_type: FileType,
    /// The Rust module path for this file, derived from the filesystem path.
    /// Empty string for the crate root (`main.rs` / `lib.rs`).
    /// For other files, segments are joined with `::` (e.g., `"dep"`, `"foo::bar"`).
    pub module_path: String,
}


/// Encodes whether or not a file requires instrumentation.
/// 
/// All files loaded by DATIR are either root files (`main.rs` or `lib.rs`` ), dependancy files
/// (imported by some other file, but within the currently compiled crate), or untracked files
/// (imported, but external to the currently compiled crate).
#[derive(Debug)]
pub enum FileType {
    /// Represents the tracked crate root file.
    Root,
    /// Represents a tracked dep file.
    Dep,
    /// Represents an untracked file.
    Untracked,
}

impl FileContents {
    /// Constructs a [`FileContents`] with appropriate metadata.
    /// 
    /// Given the source contents (usually read in from a standard rustc file loader), a path to 
    /// the file within the filesystem, and a root directory (the directory where `main.rs` or 
    /// `lib.rs` is defined), the returned [`FileContents`] will capture whether or not this
    /// file requires instrumentation.
    pub fn new(source: String, path: &std::path::Path, root_dir: Option<&std::path::Path>) -> Self {
        let path_str = path.to_str().unwrap();

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
    /// A trailing `mod` segment is dropped since `foo/mod.rs` represents
    /// the `foo` module, not `foo::mod`.
    ///
    /// Falls back to the file stem if the root dir is unknown or the path
    /// is not relative to it.
    ///
    /// # Examples:
    /// ```rust
    /// let path = "/project/tests/multi_file/dep.rs";
    /// let root_dir = Some("/project/tests/multi_file");
    /// assert_eq!(derive_module_path(path, root_dir), "dep")
    /// ```
    ///
    /// For nested paths:
    /// ```rust
    /// let path = "/project/src/foo/bar.rs";
    /// let root_dir = Some("/project/src/");
    /// assert_eq!(derive_module_path(path, root_dir), "foo::bar");
    /// ```
    fn derive_module_path(path: &std::path::Path, root_dir: Option<&std::path::Path>) -> String {
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
