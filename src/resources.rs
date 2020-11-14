use std::{env, fmt, ffi::OsString, io::Write as _, path::Path};
use tempfile::TempDir;
use which::CanonicalPath;

use crate::FatalError;
use crate::explode::ExplodePdf;
use crate::ffmpeg::Ffmpeg;

/// Command line and environment provided configuration.
pub struct Configuration {
    stdout: std::io::Stdout,
    this: Option<OsString>,
    verbose: bool,
}

pub struct Resources {
    ffmpeg: Ffmpeg,
    tempdir: TempDir,
    explode: Box<dyn ExplodePdf>,
}

pub struct RequiredToolError {
    tool: &'static str,
    error: which::Error,
}

struct ErrorReporter<'dis> {
    into: std::io::StdoutLock<'dis>,
    not_found: Vec<&'dis dyn std::fmt::Display>,
}

impl Resources {
    /// Load and inspect all required resources and optional resources and panic if it is not
    /// possible to arrive at a suitable configuration.
    pub fn force(cfg: &Configuration) -> Result<Self, FatalError> {
        // First, try and load all parts. Then give a condensed message with all missing parts.
        let ffmpeg = Ffmpeg::new();
        let tempdir = cfg.new_tempdir();
        let explode = ExplodePdf::new();

        let mut report = cfg.error_reporter();
        if let Err(err) = &ffmpeg {
            report.eat_err(err);
        }
        if let Err(err) = &tempdir {
            report.eat_err(err);
        }
        if let Err(err) = &explode {
            report.eat_err(err);
        }
        report.assert()?;

        Ok(Resources {
            ffmpeg: ffmpeg.unwrap_or_else(|_| unreachable!()),
            tempdir: tempdir.unwrap_or_else(|_| unreachable!()),
            explode: explode.unwrap_or_else(|_| unreachable!()),
        })
    }
}

impl Configuration {
    pub fn from_env() -> Result<Self, FatalError> {
        enum HowToParse {
            CurrentProgram,
            ExpectArg,
        }

        let mut cfg = Configuration {
            stdout: std::io::stdout(),
            this: None,
            verbose: false,
        };


        let mut how = HowToParse::CurrentProgram;
        for arg in env::args_os() {
            match how {
                HowToParse::CurrentProgram => cfg.this = Some(arg),
                HowToParse::ExpectArg => match arg.to_str() {
                    Some("-v") | Some("-verbose") => cfg.verbose = true,
                    Some("-h") | Some("-help") | Some("--help") => cfg.bail_help()?,
                    Some(other) => cfg.bail_unknown_argument(other)?,
                    None => cfg.bail_bad_argument(arg)?,
                }
            }

            how = HowToParse::ExpectArg;
        }

        Ok(cfg)
    }

    fn new_tempdir(&self) -> Result<TempDir, std::io::Error> {
        TempDir::new()
    }

    // TODO: want to use `Result<!, FatalError>` here.
    fn bail_unknown_argument(&mut self, arg: &str) -> Result<(), FatalError> {
        writeln!(&mut self.stdout, "Unknown argument `{}`", arg)?;
        self.print_help()?;
        std::process::exit(1);
    }

    fn bail_bad_argument(&mut self, arg: OsString) -> Result<(), FatalError> {
        writeln!(&mut self.stdout, "Os Argument is invalid `{}`", Path::new(&arg).display())?;
        std::process::exit(1);
    }

    fn bail_help(&mut self) -> Result<(), FatalError> {
        self.print_help()?;
        std::process::exit(2);
    }

    fn print_help(&mut self) -> Result<(), FatalError> {
        let (mut path, mut or_other_name);
        writeln!(&mut self.stdout, "Usage: {} [OPTION...]", {
            match &self.this {
                Some(this) => {
                    path = Path::new(this).display();
                    &mut path as &mut dyn fmt::Display
                }
                None => {
                    or_other_name = "vid-from-pdf";
                    &mut or_other_name as &mut dyn fmt::Display
                }
            }
        })?;
        writeln!(&mut self.stdout, "")?;
        writeln!(&mut self.stdout, "Options:\n\
            \t-verbose  \tPrint debug information\n\
            \t-h\n\
            \t-help\n\
            \t--help    \tPrint this help"
        )?;
        Ok(())
    }

    fn error_reporter(&self) -> ErrorReporter<'_> {
        ErrorReporter {
            into: self.stdout.lock(),
            not_found: vec![],
        }
    }
}

impl<'dis> ErrorReporter<'dis> {
    fn eat_err<E: std::fmt::Display>(&mut self, err: &'dis E) {
        self.not_found.push(err);
    }

    /// Require that no errors occurred.
    fn assert(mut self) -> Result<(), FatalError> {
        if self.not_found.is_empty() {
            return Ok(());
        }

        write!(self.into, "Some require tools could not be found or are too old. Please install them.")?;
        for err in self.not_found {
            write!(self.into, " {}", err)?;
        }
        std::process::exit(1);
    }
}

pub fn require_tool(tool: &'static str) -> Result<CanonicalPath, RequiredToolError> {
    match CanonicalPath::new(tool) {
        Ok(path) => Ok(path),
        Err(error) => Err(RequiredToolError {
            tool,
            error,
        })
    }
}

impl fmt::Display for RequiredToolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let RequiredToolError { tool, error } = self;
        write!(f, "The tool `{}` can not be used: {}", tool, error)
    }
}
