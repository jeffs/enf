use std::{
    error::Error,
    ffi,
    io::{self, Write as _},
    path::Path,
    process::Command,
};

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

fn print_usage() {
    println!(
        "\
{description}

Usage: {name} [OPTIONS] [KEY=VALUE]... [PROGRAM [ARG]...]

Options:
  -f, --file <FILE>  Set variables from a dotenv file; may be repeated
  -h, --help         Print this help",
        description = env!("CARGO_PKG_DESCRIPTION"),
        name = env!("CARGO_BIN_NAME"),
    );
}

/// Returns the vars updated by the settings, in order. Each setting replaces
/// the value of the var that has its key, or else becomes a new var at the end.
fn merge_envs<K: Into<ffi::OsString>, V: Into<ffi::OsString>>(
    mut vars: Vec<(ffi::OsString, ffi::OsString)>,
    settings: impl IntoIterator<Item = (K, V)>,
) -> Vec<(ffi::OsString, ffi::OsString)> {
    for (key, value) in settings {
        let (key, value) = (key.into(), value.into());
        match vars.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => vars.push((key, value)),
        }
    }
    vars
}

/// Prints each var to stdout as a KEY=VALUE line.
fn print_envs(vars: &[(ffi::OsString, ffi::OsString)]) -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for (key, value) in vars {
        out.write_all(key.as_encoded_bytes())?;
        out.write_all(b"=")?;
        out.write_all(value.as_encoded_bytes())?;
        out.write_all(b"\n")?;
    }
    out.flush()
}

/// Exits quietly if printing succeeded or the reader closed the pipe.
fn exit_after_print(result: io::Result<()>) -> ! {
    match result {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => {
            die(ffi::OsStr::new("stdout"), error, 4)
        }
        Ok(()) | Err(_) => std::process::exit(0),
    }
}

/// Replaces this process with the command on Unix, or else waits for it and
/// exits with its status.
fn run(cmd: &mut Command) -> ! {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let error = cmd.exec();
        // Per <https://pubs.opengroup.org/onlinepubs/9799919799/utilities/env.html#tag_20_39_14>:
        let code = if error.kind() == io::ErrorKind::NotFound {
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

fn main() {
    // Collecting owned args into a Vec lets us split KEY=VALUE into slices,
    // rather than allocating separate owned strings for KEY and VALUE.
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let enf::Args { files, envs, util } = match enf::Action::parse(&args) {
        Ok(enf::Action::Run(args)) => args,
        Ok(enf::Action::Help) => return print_usage(),
        Err(error) => {
            eprintln!("{}: {error}", env!("CARGO_BIN_NAME"));
            std::process::exit(2)
        }
    };

    // Settings from argv take precedence over settings from files.
    let file_envs = load_files(&files);
    match util {
        enf::Utility::None => {
            let vars = merge_envs(std::env::vars_os().collect(), file_envs);
            exit_after_print(print_envs(&merge_envs(vars, envs)))
        }
        enf::Utility::Argv { arg0, rest } => {
            let mut cmd = Command::new(arg0);
            cmd.args(rest).envs(file_envs).envs(envs);
            run(&mut cmd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_envs_replaces_in_place_and_appends() {
        let vars = [("A", "1"), ("B", "2")].map(|(k, v)| (k.into(), v.into()));
        let got = merge_envs(vars.into(), [("C", "3"), ("A", "4"), ("C", "5")]);
        let want = [("A", "4"), ("B", "2"), ("C", "5")].map(|(k, v)| (k.into(), v.into()));
        assert_eq!(got, want);
    }
}
