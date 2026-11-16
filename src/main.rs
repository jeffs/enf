use std::{error::Error, ffi, path::Path, process::Command};

fn die(source: &ffi::OsStr, error: impl Error, code: i32) -> ! {
    eprintln!("{}: {}: {error}", env!("CARGO_BIN_NAME"), source.display());
    std::process::exit(code)
}

/// Returns the env vars from the dotenv files, in order.
fn load_files(files: &[&Path]) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    for path in files {
        let items = match dotenvy::from_path_iter(path) {
            Ok(items) => items,
            Err(error) => die(path.as_os_str(), error, 1),
        };
        for item in items {
            match item {
                Ok(env) => envs.push(env),
                Err(error) => die(path.as_os_str(), error, 3),
            }
        }
    }
    envs
}

fn main() {
    // Collecting owned args into a Vec lets us split KEY=VALUE into slices,
    // rather than allocating separate owned strings for KEY and VALUE.
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let enf::Args { files, envs, util } = match enf::Args::parse(&args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{}: {error}", env!("CARGO_BIN_NAME"));
            std::process::exit(2)
        }
    };

    // TODO: Print the environment rather than falling back to /usr/bin/env.
    let mut cmd = match util {
        enf::Utility::None => Command::new("/usr/bin/env"),
        enf::Utility::Argv { arg0, rest } => {
            let mut cmd = Command::new(arg0);
            cmd.args(rest);
            cmd
        }
    };

    // Settings from argv take precedence over settings from files.
    cmd.envs(load_files(&files)).envs(envs);

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
        die(cmd.get_program(), error, code);
    }

    // Fallback for non-Unix platforms. We could disable the warning by
    // cfg-gating (`#[cfg(not(unix))]`), but then a Unix CI box wouldn't even
    // compile the code.
    #[cfg_attr(unix, expect(unreachable_code, reason = "unreachable only on Unix"))]
    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => die(cmd.get_program(), error, 1),
    }
}
