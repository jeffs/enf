use std::{error::Error, ffi, fmt, path::Path};

/// Splits the arg on its first `=`, if any.
///
/// # Implementation
///
/// Search and split require raw bytes instead of [`ffi::OsStr`], so we have to
/// convert to bytes and back. The conversion back is `unsafe`. Per
/// [`ffi::OsStr::from_encoded_bytes_unchecked`]:
///
/// > Due to the encoding being self-synchronizing, the bytes from
/// > `OsStr::as_encoded_bytes` can be split either immediately before or
/// > immediately after any valid non-empty UTF-8 substring.
///
/// Our separator `b'='` qualifies as a non-empty UTF-8 substring; indexes
/// `..pos` and `pos + 1..` slice the bytes immediately before and after it.
fn try_split_env(arg: &ffi::OsStr) -> Option<(&ffi::OsStr, &ffi::OsStr)> {
    let arg = arg.as_encoded_bytes();
    let pos = arg.iter().position(|b| b == &b'=')?;
    #[expect(unsafe_code, clippy::indexing_slicing, reason = "split OsStr on b'='")]
    let env = [&arg[..pos], &arg[pos + 1..]]
        .map(|s| unsafe { ffi::OsStr::from_encoded_bytes_unchecked(s) });
    Some(env.into())
}

/// A program and arguments to be executed.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Utility<'a> {
    /// No program was specified after env vars.
    #[default]
    None,
    /// Arguments specified after any env vars.
    Argv {
        /// Program specified after env vars in argv.
        arg0: &'a ffi::OsString,
        /// Remainder of argv after env vars and program.
        rest: &'a [ffi::OsString],
    },
}

/// A file option that is not followed by a path.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingPath<'a> {
    /// The option as it appears in argv.
    pub flag: &'a ffi::OsStr,
}

impl fmt::Display for MissingPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: expected path", self.flag.display())
    }
}

impl Error for MissingPath<'_> {}

/// A leading file option split from the args that follow it.
type FileOption<'a> = (&'a Path, &'a [ffi::OsString]);

/// Splits a leading `-f FILE`, `--file FILE`, or `--file=FILE` from the args.
fn try_split_file(args: &[ffi::OsString]) -> Result<Option<FileOption<'_>>, MissingPath<'_>> {
    let Some((flag, tail)) = args.split_first() else {
        return Ok(None);
    };
    if flag == "-f" || flag == "--file" {
        let (path, tail) = tail.split_first().ok_or(MissingPath { flag })?;
        return Ok(Some((Path::new(path), tail)));
    }
    Ok(try_split_env(flag)
        .filter(|(key, _)| *key == "--file")
        .map(|(_, path)| (Path::new(path), tail)))
}

/// Dotenv files, env vars, and optional utility program to run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args<'a> {
    /// Dotenv files specified by leading file options in argv.
    pub files: Vec<&'a Path>,
    /// KEY=VALUE settings specified after any file options in argv.
    pub envs: Vec<(&'a ffi::OsStr, &'a ffi::OsStr)>,
    /// Program and args specified after env vars in argv.
    pub util: Utility<'a>,
}

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Print usage, as requested by a leading `-h` or `--help`.
    Help,
    /// Run a utility program.
    Run(Args<'a>),
}

impl<'a> Action<'a> {
    /// Returns the action specified by argv.
    ///
    /// # Errors
    ///
    /// Returns [`MissingPath`] if a file option is the last arg.
    pub fn parse(mut args: &'a [ffi::OsString]) -> Result<Self, MissingPath<'a>> {
        let mut files = Vec::new();
        loop {
            if args
                .first()
                .is_some_and(|arg| arg == "-h" || arg == "--help")
            {
                return Ok(Self::Help);
            }
            let Some((file, tail)) = try_split_file(args)? else {
                break;
            };
            files.push(file);
            args = tail;
        }
        Ok(Self::Run(Args::parse(files, args)))
    }
}

impl<'a> Args<'a> {
    /// Returns the specified files, with env vars and optional utility program
    /// (and args) from the args that follow any options.
    fn parse(files: Vec<&'a Path>, args: &'a [ffi::OsString]) -> Self {
        let mut envs = Vec::new();
        let mut program = None;
        for arg in args {
            if let Some(env) = try_split_env(arg) {
                envs.push(env);
            } else {
                program = Some(arg);
                break;
            }
        }
        let util = program.map_or(Utility::None, |arg0| {
            let Some(rest) = args.get(envs.len() + 1..) else {
                unreachable!("envs and program are a prefix of args");
            };
            Utility::Argv { arg0, rest }
        });
        Self { files, envs, util }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envs(
        items: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Vec<(&'static ffi::OsStr, &'static ffi::OsStr)> {
        items
            .into_iter()
            .map(|(k, v)| (ffi::OsStr::new(k), ffi::OsStr::new(v)))
            .collect()
    }

    #[test]
    fn parse_args_works() {
        for (args, want) in [
            (&[][..], Args::default()),
            (
                &["ANSWER=42"],
                Args {
                    envs: envs([("ANSWER", "42")]),
                    ..Args::default()
                },
            ),
            (
                &["ANSWER=42", "echo"],
                Args {
                    envs: envs([("ANSWER", "42")]),
                    util: Utility::Argv {
                        arg0: &"echo".into(),
                        rest: &[],
                    },
                    ..Args::default()
                },
            ),
            (
                &["ANSWER=42", "SUM=41", "echo", "hello", "world"],
                Args {
                    envs: envs([("ANSWER", "42"), ("SUM", "41")]),
                    util: Utility::Argv {
                        arg0: &"echo".into(),
                        rest: &["hello".into(), "world".into()],
                    },
                    ..Args::default()
                },
            ),
        ] {
            let args = args.iter().map(ffi::OsString::from).collect::<Vec<_>>();
            let got = Action::parse(&args);
            assert_eq!(got, Ok(Action::Run(want)));
        }
    }

    #[test]
    fn parse_args_splits_file_options() {
        let args = [
            "-f",
            "a.env",
            "--file",
            "b.env",
            "--file=c=d.env",
            "K=V",
            "echo",
            "-f",
        ]
        .map(ffi::OsString::from);
        let want = Args {
            files: ["a.env", "b.env", "c=d.env"].map(Path::new).into(),
            envs: envs([("K", "V")]),
            util: Utility::Argv {
                arg0: &"echo".into(),
                rest: &["-f".into()],
            },
        };
        assert_eq!(Action::parse(&args), Ok(Action::Run(want)));
    }

    #[test]
    fn parse_args_recognizes_leading_help() {
        for args in [
            &["-h"][..],
            &["--help"],
            &["-f", "a.env", "-h", "echo"],
            &["--help", "-f"],
        ] {
            let args = args.iter().map(ffi::OsString::from).collect::<Vec<_>>();
            assert_eq!(Action::parse(&args), Ok(Action::Help));
        }
    }

    #[test]
    fn parse_args_passes_later_help_to_utility() {
        for args in [&["K=V", "-h"][..], &["echo", "--help"], &["-f", "-h"]] {
            let args = args.iter().map(ffi::OsString::from).collect::<Vec<_>>();
            assert!(matches!(Action::parse(&args), Ok(Action::Run(_))));
        }
    }

    #[test]
    fn parse_args_rejects_file_option_without_path() {
        for flag in ["-f", "--file"] {
            let args = ["-f", "a.env", flag].map(ffi::OsString::from);
            let want = MissingPath {
                flag: ffi::OsStr::new(flag),
            };
            assert_eq!(Action::parse(&args), Err(want));
        }
    }
}
