use std::{error::Error, ffi, iter, process::Command};

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

fn die(program: &ffi::OsStr, error: impl Error, code: i32) -> ! {
    eprintln!("{}: {}: {error}", env!("CARGO_BIN_NAME"), program.display());
    std::process::exit(code)
}

fn main() {
    // Collecting owned args into a Vec lets us split KEY=VALUE into slices,
    // rather than allocating separate owned strings for KEY and VALUE.
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let mut args = args.iter().peekable();
    let envs: Vec<_> = iter::from_fn(|| args.next_if_map(|s| try_split_env(s).ok_or(s))).collect();

    // TODO: Print the environment rather than falling back to /usr/bin/env.
    let program = args
        .next()
        .map_or_else(|| ffi::OsStr::new("/usr/bin/env"), ffi::OsString::as_os_str);

    let mut cmd = Command::new(program);
    let cmd = cmd.envs(envs).args(args);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let error = cmd.exec();
        // Per <https://pubs.opengroup.org/onlinepubs/9799919799/utilities/env.html#tag_20_39_14>:
        let code = if error.kind() == std::io::ErrorKind::NotFound {
            127 // The utility could not be found.
        } else {
            126 // The utility was found but could not be invoked.
        };
        die(program, error, code);
    }

    // Fallback for non-Unix platforms. We could disable the warning by
    // cfg-gating (`#[cfg(not(unix))]`), but then a Unix CI box wouldn't even
    // compile the code.
    #[cfg_attr(unix, expect(unreachable_code, reason = "unreachable only on Unix"))]
    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => die(program, error, 1),
    }
}
