use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// boolean flag, mostly for specifying --test
    Flag,
    /// keyword arg, e.g. -o hello --> {"o": "hello"}
    Keyword,
    /// regular arg, DATIR only supports a single one
    Positional,
}

/// Specification for a single command-line argument.
#[derive(Clone, Debug)]
pub struct ArgSpec {
    /// Internal name used to look the argument up after parsing
    pub name: &'static str,
    /// short form arg name e.g. `-o`
    pub short: Option<&'static str>,
    /// long form e.g. `--output`
    pub long: Option<&'static str>,
    /// The kind of argument, a flag, keyword, or positional
    pub kind: ArgKind,
    /// whether the argument must be supplied
    pub required: bool,
    /// short help string displayed in usage
    pub help: &'static str,
    /// placeholder shown in usage for non-flag values (e.g. `PATH`)
    pub value_name: Option<&'static str>,
}

impl ArgSpec {
    /// Constructs a flag argument
    pub fn flag(name: &'static str, long: &'static str, help: &'static str) -> Self {
        Self {
            name,
            short: None,
            long: Some(long),
            kind: ArgKind::Flag,
            required: false,
            help,
            value_name: None,
        }
    }

    /// Constructs a keyword based argument
    pub fn keyword(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            short: None,
            long: None,
            kind: ArgKind::Keyword,
            required: false,
            help,
            value_name: Some("VALUE"),
        }
    }

    /// Constructs a positional argument
    pub fn positional(name: &'static str, value_name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            short: None,
            long: None,
            kind: ArgKind::Positional,
            required: true,
            help,
            value_name: Some(value_name),
        }
    }

    /// Sets the short form name of the argument
    pub fn short(mut self, s: &'static str) -> Self {
        self.short = Some(s);
        self
    }

    /// Sets the long form name of the argument
    pub fn long(mut self, l: &'static str) -> Self {
        self.long = Some(l);
        self
    }

    /// Sets the usage placeholder value for this argument
    pub fn value_name(mut self, v: &'static str) -> Self {
        self.value_name = Some(v);
        self
    }

    /// Makes this argument required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// Errors that can result from argument parsing
#[derive(Debug)]
pub enum ArgError {
    MissingRequired(&'static str),
    UnknownFlag(String),
    MissingValue(String),
    UnexpectedPositional(String),
    DuplicatePositional { first: String, second: String },
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgError::MissingRequired(n) => write!(f, "missing required argument `{n}`"),
            ArgError::UnknownFlag(s) => write!(f, "unknown flag `{s}`"),
            ArgError::MissingValue(s) => write!(f, "flag `{s}` requires a value"),
            ArgError::UnexpectedPositional(s) => {
                write!(f, "unexpected positional argument `{s}`")
            }
            ArgError::DuplicatePositional { first, second } => write!(
                f,
                "multiple positional arguments provided: already have `{first}`, got `{second}`"
            ),
        }
    }
}

impl std::error::Error for ArgError {}

#[derive(Debug, Default)]
pub struct ParsedArgs {
    values: HashMap<&'static str, String>,
    flags: HashSet<&'static str>,
}

impl ParsedArgs {
    /// Returns whether the named flag was set (or an option was provided).
    pub fn is_present(&self, name: &str) -> bool {
        self.flags.contains(name) || self.values.contains_key(name)
    }

    /// Returns the value associated with an option/positional argument.
    pub fn get_value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}

/// Declarative argument parser.
pub struct ArgParser {
    program: String,
    about: &'static str,
    specs: Vec<ArgSpec>,
}

impl ArgParser {
    pub fn new(program: impl Into<String>, about: &'static str) -> Self {
        Self {
            program: program.into(),
            about,
            specs: Vec::new(),
        }
    }

    /// Register an allowed argument.
    pub fn arg(mut self, spec: ArgSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Parse `std::env::args()`, skipping `argv[0]`. On error prints usage to
    /// stderr and exits. On `--help`/`-h` prints usage to stdout and exits.
    pub fn parse_env(&self) -> ParsedArgs {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        self.parse_or_exit(raw)
    }

    /// Parse the given argument list, handling `--help` / `-h` and errors by
    /// printing usage and exiting.
    pub fn parse_or_exit<I>(&self, raw: I) -> ParsedArgs
    where
        I: IntoIterator<Item = String>,
    {
        let raw: Vec<String> = raw.into_iter().collect();
        if raw.iter().any(|a| a == "--help" || a == "-h") {
            println!("{}", self.usage());
            std::process::exit(0);
        }
        match self.parse(raw) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("ERROR: {e}\n\n{}", self.usage());
                std::process::exit(2);
            }
        }
    }

    /// Parse the given argument list.
    pub fn parse<I>(&self, raw: I) -> Result<ParsedArgs, ArgError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut parsed = ParsedArgs::default();
        let mut iter = raw.into_iter();

        while let Some(arg) = iter.next() {
            if arg.starts_with('-') {
                let spec = self
                    .specs
                    .iter()
                    .find(|s| s.short == Some(arg.as_str()) || s.long == Some(arg.as_str()))
                    .ok_or_else(|| ArgError::UnknownFlag(arg.clone()))?;

                match spec.kind {
                    ArgKind::Flag => {
                        parsed.flags.insert(spec.name);
                    }
                    ArgKind::Keyword => {
                        let value = iter.next().ok_or_else(|| ArgError::MissingValue(arg))?;
                        parsed.values.insert(spec.name, value);
                    }
                    ArgKind::Positional => { unreachable!(); }
                }
            } else {
                let spec = self
                    .specs
                    .iter()
                    .find(|s| s.kind == ArgKind::Positional)
                    .ok_or_else(|| ArgError::UnexpectedPositional(arg.clone()))?;

                if let Some(first) = parsed.values.get(spec.name) {
                    return Err(ArgError::DuplicatePositional {
                        first: first.clone(),
                        second: arg,
                    });
                }

                parsed.values.insert(spec.name, arg);
            }
        }

        // Check for missing required args
        for spec in &self.specs {
            if spec.required && !parsed.is_present(spec.name) {
                return Err(ArgError::MissingRequired(spec.name));
            }
        }

        Ok(parsed)
    }

    /// Render a usage string from the registered specs.
    pub fn usage(&self) -> String {
        let mut out = String::new();
        out.push_str(self.about);
        out.push_str("\n\n");

        // Usage line.
        out.push_str(&format!("Usage: {} [OPTIONS]", self.program));
        for spec in self.specs.iter().filter(|s| s.kind == ArgKind::Positional) {
            let ph = spec.value_name.unwrap_or(spec.name);
            if spec.required {
                out.push_str(&format!(" <{ph}>"));
            } else {
                out.push_str(&format!(" [{ph}]"));
            }
        }
        out.push_str("\n\n");

        // Arguments section.
        let positionals: Vec<_> = self
            .specs
            .iter()
            .filter(|s| s.kind == ArgKind::Positional)
            .collect();
        if !positionals.is_empty() {
            out.push_str("Arguments:\n");
            for s in positionals {
                let ph = s.value_name.unwrap_or(s.name);
                out.push_str(&format!("  <{ph}> {}\n", s.help));
            }
            out.push('\n');
        }

        // Options section.
        out.push_str("Options:\n");
        for s in self
            .specs
            .iter()
            .filter(|s| s.kind != ArgKind::Positional)
        {
            let mut left = String::new();
            if let Some(short) = s.short {
                left.push_str(short);
                if s.long.is_some() {
                    left.push_str(", ");
                }
            } else {
                left.push_str("    ");
            }
            if let Some(long) = s.long {
                left.push_str(long);
            }
            if s.kind == ArgKind::Keyword {
                let ph = s.value_name.unwrap_or("VALUE");
                left.push_str(&format!(" <{ph}>"));
            }
            out.push_str(&format!("  {left:<28} {}\n", s.help));
        }
        out.push_str(&format!("  {:<28} {}\n", "-h, --help", "Print this help"));

        out
    }
}
