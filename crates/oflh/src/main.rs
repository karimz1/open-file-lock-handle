#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
use std::{ffi::OsString, io::IsTerminal, process::ExitCode};
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Core(#[from] oflh_core::Error),
    #[error(transparent)]
    Terminal(#[from] std::io::Error),
    #[error("an interactive terminal is required; run oflh . in a terminal")]
    NotInteractive,
}

#[derive(Default)]
struct Arguments {
    path: Option<OsString>,
    view: oflh_tui::StartOptions,
    help: bool,
    version: bool,
}

fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Arguments, CliError> {
    let mut result = Arguments::default();
    let mut positional = false;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if !positional {
            if argument == "--" {
                positional = true;
                continue;
            }
            if argument == "--help" || argument == "-h" || argument == "-help" {
                result.help = true;
                return Ok(result);
            }
            if argument == "--version" || argument == "-version" {
                result.version = true;
                return Ok(result);
            }
            if argument == "--ports" {
                result.view.ports = true;
                continue;
            }
            if argument == "--port" {
                let port = arguments
                    .next()
                    .and_then(|value| value.to_str().and_then(|text| text.parse::<u16>().ok()))
                    .filter(|port| *port > 0)
                    .ok_or_else(|| {
                        CliError::Usage("--port requires a number from 1 to 65535".into())
                    })?;
                result.view.ports = true;
                result.view.port = Some(port);
                continue;
            }
            if argument.to_string_lossy().starts_with('-') {
                return Err(CliError::Usage(format!(
                    "unknown option: {}",
                    argument.to_string_lossy()
                )));
            }
        }
        if result.path.replace(argument).is_some() {
            return Err(CliError::Usage(
                "expected one path; quote paths containing spaces".into(),
            ));
        }
        positional = true;
    }
    result.view.follow_port_folder = result.path.is_none();
    result.view.ports_path_only = result.view.ports && result.path.is_some();
    Ok(result)
}

fn run() -> Result<(), CliError> {
    let arguments = parse_arguments(std::env::args_os().skip(1))?;
    if arguments.help {
        println!(
            "oflh — Open File Lock Handle. See what's using your files and ports.
Source: https://github.com/karimz1/open-file-lock-handle

Usage: oflh [OPTIONS] [PATH]

  oflh .                  inspect files in the current directory
  oflh --ports            inspect all visible local port bindings
  oflh --port 3000        find a TCP listener or UDP binding on port 3000
  oflh --ports .          inspect ports of processes using this directory

No PATH starts in the current directory; inspecting a port follows its owner folder.
An explicit PATH scopes Ports and keeps the file-inspection folder fixed.
Press s in Ports to switch between This path and All ports. Put options before PATH.

Options:
  --ports       start in the Ports tab (TCP listeners and bound UDP)
  --port PORT   start in Ports with an exact local-port filter
  --version     print version
  --help        show help"
        );
        return Ok(());
    }
    if arguments.version {
        println!("{}", oflh_core::version_report("oflh"));
        return Ok(());
    }
    let path = arguments.path;
    let target = oflh_core::Target::new(path.unwrap_or_else(|| OsString::from(".")))?;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(CliError::NotInteractive);
    }
    let backend = oflh_platform::native()?;
    oflh_tui::run(
        target,
        oflh_core::display_version().into(),
        backend,
        arguments.view,
    )?;
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = if matches!(error, CliError::Usage(_)) {
                2
            } else {
                1
            };
            eprintln!("oflh: {}", oflh_core::safe(&error.to_string()));
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_port_paths_start_scoped_and_omitted_paths_start_global() {
        for (arguments, scoped) in [
            (vec!["--ports"], false),
            (vec!["--port", "5040"], false),
            (vec!["--ports", "./project"], true),
            (vec!["--port", "5040", "./project"], true),
            (vec!["--ports", "."], true),
        ] {
            let parsed = parse_arguments(arguments.into_iter().map(OsString::from)).unwrap();
            assert_eq!(parsed.view.ports_path_only, scoped);
            assert_eq!(parsed.view.follow_port_folder, !scoped);
        }
    }

    #[test]
    fn port_options_and_literal_paths() {
        let parse = |args: &[&str]| parse_arguments(args.iter().map(OsString::from));
        let args = parse(&["--port", "3000", "."]).unwrap();
        assert!(args.view.ports);
        assert!(!args.view.follow_port_folder);
        assert!(parse(&["--port", "5040"]).unwrap().view.follow_port_folder);
        assert!(
            !parse(&["--port", "5040", "."])
                .unwrap()
                .view
                .follow_port_folder
        );
        assert!(parse(&["--here"]).is_err());
        assert_eq!(args.view.port, Some(3000));
        assert_eq!(args.path, Some(OsString::from(".")));
        for args in [
            &["--port"][..],
            &["--port", "0"],
            &["--port", "65536"],
            &["--port", "abc"],
        ] {
            assert!(parse(args).is_err());
        }
        assert_eq!(
            parse(&["--", "--ports"]).unwrap().path,
            Some(OsString::from("--ports"))
        );
        assert!(!parse(&["3000"]).unwrap().view.ports);
    }
}
