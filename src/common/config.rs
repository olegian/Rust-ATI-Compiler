/* This file defines the global configuration and logging setup used throughout DATIR
*/

use std::path::Path;
use std::io::Write;

/// DATIR configuration, to provide easy access to some helpful debugging information.
pub struct DatirConfig {
    /// Directory where all log files are written. If None, all debug output is writen to stdout.
    pub log_dir: Option<Box<Path>>,
    /// Whether or not to print out the modified source code after the second compiler invocation.
    pub print_transformed_source: bool,
}

impl DatirConfig {
    /// Simple configuration intended to be used for debugging.
    pub fn debug(log_dir: Option<Box<Path>>) -> Self {
        // make sure log directory exists and is empty
        // if let Some(dir) = &log_dir {
        //     let _ = std::fs::remove_dir_all(dir);
        //     std::fs::create_dir_all(dir);
        // }

        Self {
            log_dir,
            print_transformed_source: true
        }
    }

    /// Simple configuration intended to be used for consumer use
    pub fn release() -> Self {
        Self {
            log_dir: None,
            print_transformed_source: false,
        }
    }

    /// Logs the message, giving it a prefix to make it easier to identify.
    /// If self.log_dir is set, then the prefix becomes the name of the file which
    /// will be appended to.
    pub fn log(&self, prefix: &'static str, message: &str) {
        match &self.log_dir {
            Some(dir) => {
                let mut log_file_path = dir.to_path_buf();
                log_file_path.push(prefix);

                let mut file = std::fs::OpenOptions::new().append(true).create(true).open(log_file_path).unwrap();
                let _ = writeln!(file, "{}", message);
            },
            None => {
                println!("[{prefix}]: {message}");
            },
        }
    }
}
