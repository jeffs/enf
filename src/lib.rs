use std::ffi;

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

/// Env vars, program name, and remaining args from argv.
///
/// TODO: Make the state with no program  but some rest args unrepresentable.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args<'a> {
    /// Leading KEY=VALUE settings from argv.
    pub envs: Vec<(&'a ffi::OsStr, &'a ffi::OsStr)>,
    /// Program specified after env vars in argv.
    pub program: Option<&'a ffi::OsString>,
    /// Remainder of argv after env vars and program.
    pub rest: &'a [ffi::OsString],
}

impl<'a> Args<'a> {
    /// Returns env vars, program name, and remaining args.
    #[must_use]
    pub fn parse(args: &'a [ffi::OsString]) -> Self {
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
        let Some(rest) = args.get(envs.len() + program.iter().len()..) else {
            unreachable!("envs and program are a prefix of args");
        };
        Self {
            envs,
            program,
            rest,
        }
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
                    program: Some(&"echo".into()),
                    ..Args::default()
                },
            ),
            (
                &["ANSWER=42", "SUM=41", "echo", "hello", "world"],
                Args {
                    envs: envs([("ANSWER", "42"), ("SUM", "41")]),
                    program: Some(&"echo".into()),
                    rest: &["hello".into(), "world".into()],
                },
            ),
        ] {
            let args = args.iter().map(ffi::OsString::from).collect::<Vec<_>>();
            let got = Args::parse(&args);
            assert_eq!(got, want);
        }
    }
}
