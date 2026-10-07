#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
mod messages;
use oflh_tui::Language;
use std::{ffi::OsString, io::IsTerminal, process::ExitCode};
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Core(#[from] oflh_core::Error),
    #[error(transparent)]
    Terminal(#[from] std::io::Error),
    #[error("{0}")]
    NotInteractive(&'static str),
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
                continue;
            }
            if argument == "--version" || argument == "-version" {
                result.version = true;
                continue;
            }
            if argument == "--language" {
                let value = arguments
                    .next()
                    .ok_or_else(|| language_usage(result.view.language))?;
                result.view.language = match value.to_str() {
                    Some("system") => None,
                    Some(value) => Some(
                        value
                            .parse()
                            .map_err(|_| language_usage(result.view.language))?,
                    ),
                    None => return Err(language_usage(result.view.language)),
                };
                continue;
            }
            if argument == "--no-update-check" {
                result.view.no_update_check = true;
                continue;
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
                        CliError::Usage(
                            messages::choose(
                                result.view.language.unwrap_or_else(Language::system),
                                "--port requires a number from 1 to 65535",
                                "--port benötigt eine Zahl von 1 bis 65535",
                                "--port 需要 1 到 65535 之间的数字",
                            )
                            .into(),
                        )
                    })?;
                result.view.ports = true;
                result.view.port = Some(port);
                continue;
            }
            if argument.to_string_lossy().starts_with('-') {
                return Err(CliError::Usage(format!(
                    "{}: {}",
                    messages::choose(
                        result.view.language.unwrap_or_else(Language::system),
                        "unknown option",
                        "unbekannte Option",
                        "未知选项"
                    ),
                    argument.to_string_lossy()
                )));
            }
        }
        if result.path.replace(argument).is_some() {
            return Err(CliError::Usage(
                messages::choose(
                    result.view.language.unwrap_or_else(Language::system),
                    "expected one path; quote paths containing spaces",
                    "Genau einen Pfad angeben; Pfade mit Leerzeichen in Anführungszeichen setzen",
                    "只接受一个路径；包含空格的路径需加引号",
                )
                .into(),
            ));
        }
        positional = true;
    }
    result.view.follow_port_folder = result.path.is_none();
    result.view.ports_path_only = result.view.ports && result.path.is_some();
    Ok(result)
}

fn language_usage(language: Option<Language>) -> CliError {
    CliError::Usage(
        messages::choose(
            language.unwrap_or_else(Language::system),
            "--language requires en, de, zh or system",
            "--language benötigt en, de, zh oder system",
            "--language 需要 en、de、zh 或 system",
        )
        .into(),
    )
}

fn run() -> Result<(), CliError> {
    if let Some(result) = oflh_platform::inspection_helper::dispatch() {
        return Ok(result?);
    }
    let arguments = parse_arguments(std::env::args_os().skip(1))?;
    if arguments.help {
        println!(
            "{}",
            messages::help(arguments.view.language.unwrap_or_else(Language::system))
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
        return Err(CliError::NotInteractive(messages::choose(
            arguments.view.language.unwrap_or_else(Language::system),
            "an interactive terminal is required; run oflh . in a terminal",
            "Ein interaktives Terminal wird benötigt; oflh . im Terminal starten",
            "需要交互式终端；请在终端中运行 oflh .",
        )));
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
    fn disabling_update_checks_preserves_options_and_literal_path_rules() {
        let parsed = parse_arguments(
            ["--no-update-check", "--language", "de", "--ports", "."]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(parsed.view.no_update_check);
        assert!(parsed.view.ports);
        assert_eq!(parsed.view.language, Some(Language::German));
        assert_eq!(parsed.path, Some(OsString::from(".")));
        let literal =
            parse_arguments(["--", "--no-update-check"].into_iter().map(OsString::from)).unwrap();
        assert!(!literal.view.no_update_check);
        assert_eq!(literal.path, Some(OsString::from("--no-update-check")));
    }
    #[test]
    fn explicit_language_and_system_options_preserve_path_and_help_arguments() {
        for (value, expected) in [
            ("en", Some(Language::English)),
            ("de", Some(Language::German)),
            ("zh", Some(Language::Chinese)),
            ("system", None),
        ] {
            let parsed = parse_arguments(
                ["--help", "--language", value]
                    .into_iter()
                    .map(OsString::from),
            )
            .unwrap();
            assert!(parsed.help);
            assert_eq!(parsed.view.language, expected);
            let parsed =
                parse_arguments(["--language", value, "."].into_iter().map(OsString::from))
                    .unwrap();
            assert_eq!(parsed.path, Some(OsString::from(".")));
            assert_eq!(parsed.view.language, expected);
        }
        for values in [
            vec!["--language"],
            vec!["--language", "fr"],
            vec!["--language", "zh_CN"],
        ] {
            assert!(parse_arguments(values.into_iter().map(OsString::from)).is_err());
        }
    }
    #[cfg(windows)]
    #[test]
    fn drive_root_arguments_preserve_native_text_and_resolve_as_directories() {
        for root in [r"C:\", r"\\?\C:\"] {
            let arguments =
                parse_arguments([OsString::from("--ports"), OsString::from(root)]).unwrap();
            assert_eq!(arguments.path, Some(OsString::from(root)));
            let target = oflh_core::Target::new(arguments.path.unwrap()).unwrap();
            assert!(target.directory);
            assert_eq!(oflh_core::display_path(&target.path), r"C:\");
            assert!(arguments.view.ports_path_only);
            assert!(!arguments.view.follow_port_folder);
        }
    }
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
