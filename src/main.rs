use std::{error::Error, ffi, process::Command};

fn die(program: &ffi::OsStr, error: impl Error, code: i32) -> ! {
    eprintln!("{}: {}: {error}", env!("CARGO_BIN_NAME"), program.display());
    std::process::exit(code)
}

fn main() {
    // Collecting owned args into a Vec lets us split KEY=VALUE into slices,
    // rather than allocating separate owned strings for KEY and VALUE.
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let enf::Args {
        envs,
        program,
        rest,
    } = enf::Args::parse(&args[..]);

    // TODO: Print the environment rather than falling back to /usr/bin/env.
    let program = program.map_or_else(|| ffi::OsStr::new("/usr/bin/env"), ffi::OsString::as_os_str);

    let mut cmd = Command::new(program);
    let cmd = cmd.envs(envs).args(rest);

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
