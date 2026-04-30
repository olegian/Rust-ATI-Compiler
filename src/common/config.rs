/* This file defines the global configuration and logging setup used throughout DATIR
*/

use std::io::Write;
use std::path::Path;

/// DATIR configuration, to provide easy access to some helpful debugging information.
#[derive(Debug)]
pub struct DatirConfig {
    /// Directory where all log files are written. If None, all debug output is writen to stdout.
    pub log_dir: Option<Box<Path>>,
    /// Whether or not to print out the modified source code after the second compiler invocation.
    pub print_transformed_source: bool,
    /// Whether or not to print the information gathered from the first pass
    pub print_first_pass_info: bool,
    /// Whether or not to print the information regarding function signatures used to create
    /// function stubs
    pub print_function_signatures: bool,
    /// Whether or not to output the config after it is constructed.
    pub print_config: bool,
    /// Directory to which to output .ati files. If None, instrumented binary will 
    /// print ATI related output to stdout.
    pub ati_output_dir: Option<std::path::PathBuf>,
    /// DeclsFile associated with crate being instrumented.
    pub decls_file: decls_gen::DeclsFile,
}

impl DatirConfig {
    /// Simple configuration intended to be used for debugging.
    pub fn debug(decls_file: decls_gen::DeclsFile) -> Self {
        // make sure log directory exists and is empty
        // FIXME: have the final executable also be created in this directory when using debug
        let cwd = std::env::current_dir().unwrap();
        let log_dir = cwd.join("logs").into_boxed_path();
        let _ = std::fs::remove_dir_all(&log_dir);
        let _ = std::fs::create_dir_all(&log_dir);

        Self {
            log_dir: Some(log_dir),
            print_transformed_source: true,
            print_first_pass_info: true,
            print_function_signatures: true,
            print_config: true,
            ati_output_dir: None,
            decls_file,
        }
    }

    pub fn test(decls_file: decls_gen::DeclsFile) -> Self {
        Self {
            log_dir: None,
            print_transformed_source: false,
            print_first_pass_info: false,
            print_function_signatures: false,
            print_config: false,
            ati_output_dir: None,
            decls_file,
        }
    }

    /// Simple configuration intended to be used for consumer use
    pub fn release(decls_file: decls_gen::DeclsFile, ati_output_dir: std::path::PathBuf) -> Self {
        Self {
            log_dir: None,
            print_transformed_source: false,
            print_first_pass_info: false,
            print_function_signatures: false,
            print_config: false,
            ati_output_dir: Some(ati_output_dir),
            decls_file
        }
    }

    /// Logs the message, giving it a prefix to make it easier to identify.
    /// If self.log_dir is set, then the prefix becomes the name of the file which
    /// will be appended to.
    pub fn log(&self, prefix: &'static str, message: String) {
        match &self.log_dir {
            Some(dir) => {
                let mut log_file_path = dir.to_path_buf();
                log_file_path.push(prefix);

                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_file_path)
                    .unwrap();
                let _ = writeln!(file, "{}", message);
            }
            None => {
                println!("[{prefix}]: {message}");
            }
        }
    }
}
