//! `cargo xtask build-installer`: a local desktop installer for the host OS.
//!
//! Release installers come from CI (`.github/workflows/desktop.yml`). This command
//! is a developer convenience: it installs the frontend dependencies when needed,
//! runs the Tauri build for the host's default bundles, and prints the packages.
use super::Result;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

pub(super) const USAGE: &str =
    "usage: cargo xtask build-installer [--bundles LIST] [--clean] [--signed]";

/// Options for one `build-installer` run.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Options {
    /// Comma-separated Tauri bundle list; `None` uses the host default.
    bundles: Option<String>,
    /// Run `npm ci` even when `node_modules` exists.
    clean: bool,
    /// Create signed updater artifacts (needs `TAURI_SIGNING_PRIVATE_KEY`).
    signed: bool,
}

pub(super) fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Options> {
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--bundles" => {
                let bundles = arguments.next().ok_or("--bundles requires a value")?;
                if bundles.is_empty()
                    || !bundles.split(',').all(|bundle| {
                        !bundle.is_empty()
                            && bundle.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    })
                {
                    return Err(
                        "--bundles takes a comma-separated list such as nsis or deb,rpm".into(),
                    );
                }
                if options.bundles.replace(bundles).is_some() {
                    return Err("duplicate option --bundles".into());
                }
            }
            "--clean" => options.clean = true,
            "--signed" => options.signed = true,
            "--help" | "-h" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n{USAGE}").into()),
        }
    }
    Ok(options)
}

/// The bundles CI builds for each OS (see the desktop workflow matrix).
fn default_bundles(os: &str) -> Result<&'static str> {
    match os {
        "windows" => Ok("nsis"),
        "linux" => Ok("deb,rpm"),
        "macos" => Ok("app,dmg"),
        other => Err(format!("no default installer bundles for {other}; pass --bundles").into()),
    }
}

/// npm is a batch script on Windows; `Command` does not try `.cmd` on its own.
fn npm_program(os: &str) -> &'static str {
    if os == "windows" { "npm.cmd" } else { "npm" }
}

/// Updater artifacts must be signed, so they are built only on request or when
/// a signing key is configured; otherwise a local build without the key fails.
fn creates_updater_artifacts(signed: bool, signing_key_set: bool) -> bool {
    signed || signing_key_set
}

/// Tauri merges this file over `tauri.conf.json`.
const UNSIGNED_CONFIG: &str = "{\"bundle\":{\"createUpdaterArtifacts\":false}}\n";

fn tauri_arguments(bundles: &str, config: Option<&Path>) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["run", "tauri", "--", "build", "--bundles", bundles]
        .into_iter()
        .map(OsString::from)
        .collect();
    if let Some(config) = config {
        arguments.push("--config".into());
        arguments.push(config.as_os_str().to_owned());
    }
    arguments.extend(["--", "--locked"].map(OsString::from));
    arguments
}

/// Directory below `target/release/bundle` that Tauri writes each bundle to.
fn bundle_directory(bundle: &str) -> &str {
    match bundle {
        "app" => "macos",
        other => other,
    }
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask has no parent directory".into())
}

fn target_directory(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(directory) => root.join(directory),
        None => root.join("target"),
    }
}

fn run(command: &mut Command, description: &str) -> Result<()> {
    let status = command
        .status()
        .map_err(|error| format!("could not start {description}: {error}"))?;
    if !status.success() {
        return Err(format!("{description} failed: {status}").into());
    }
    Ok(())
}

/// Packages in `bundle/<kind>` written since `started`, so older builds are not reported.
/// Packages are files, plus macOS `.app` bundles; other directories are Tauri's staging.
fn new_packages(bundle_root: &Path, bundles: &str, started: SystemTime) -> Result<Vec<PathBuf>> {
    let mut packages = Vec::new();
    for bundle in bundles.split(',') {
        let directory = bundle_root.join(bundle_directory(bundle));
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            let package = metadata.is_file()
                || (metadata.is_dir()
                    && path.extension().is_some_and(|extension| extension == "app"));
            if package && metadata.modified()? >= started {
                packages.push(path);
            }
        }
    }
    packages.sort();
    Ok(packages)
}

pub(super) fn build(options: Options) -> Result<()> {
    let os = std::env::consts::OS;
    let bundles = match &options.bundles {
        Some(bundles) => bundles.as_str(),
        None => default_bundles(os)?,
    };
    let signing_key_set =
        std::env::var_os("TAURI_SIGNING_PRIVATE_KEY").is_some_and(|key| !key.is_empty());
    if options.signed && !signing_key_set {
        return Err("--signed needs TAURI_SIGNING_PRIVATE_KEY (see docs/releasing.md)".into());
    }
    let root = workspace_root()?;
    let ui = root.join("crates/oflh-desktop/ui");
    let npm = npm_program(os);
    if options.clean || !ui.join("node_modules").is_dir() {
        run(Command::new(npm).arg("ci").current_dir(&ui), "npm ci")?;
    }
    let target = target_directory(&root);
    let config = if creates_updater_artifacts(options.signed, signing_key_set) {
        None
    } else {
        let path = target.join("xtask/build-installer.tauri.json");
        fs::create_dir_all(path.parent().ok_or("config path has no parent")?)?;
        fs::write(&path, UNSIGNED_CONFIG)?;
        println!("Updater artifacts disabled (no TAURI_SIGNING_PRIVATE_KEY, no --signed).");
        Some(path)
    };
    let started = SystemTime::now();
    run(
        Command::new(npm)
            .args(tauri_arguments(bundles, config.as_deref()))
            .current_dir(&ui),
        "tauri build",
    )?;
    let bundle_root = target.join("release/bundle");
    let packages = new_packages(&bundle_root, bundles, started)?;
    if packages.is_empty() {
        println!(
            "Build finished; look for packages in {}",
            bundle_root.display()
        );
    } else {
        println!("Installer packages:");
        for package in packages {
            println!("  {}", package.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(values: &[&str]) -> Result<Options> {
        parse(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn options_accept_bundles_clean_and_signed() {
        assert_eq!(parse_all(&[]).unwrap(), Options::default());
        assert_eq!(
            parse_all(&["--bundles", "deb,rpm", "--clean", "--signed"]).unwrap(),
            Options {
                bundles: Some("deb,rpm".into()),
                clean: true,
                signed: true,
            }
        );
        for invalid in [
            &["--bundles"][..],
            &["--bundles", ""],
            &["--bundles", "deb,"],
            &["--bundles", "deb;rm"],
            &["--bundles", "nsis", "--bundles", "msi"],
            &["--version", "dev"],
        ] {
            assert!(parse_all(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn host_defaults_match_the_desktop_workflow_matrix() {
        let workflow = include_str!("../../.github/workflows/desktop.yml");
        for (os, workflow_os) in [
            ("windows", "windows"),
            ("linux", "linux"),
            ("macos", "darwin"),
        ] {
            let bundles = default_bundles(os).unwrap();
            let quoted = if bundles.contains(',') {
                format!("bundles: '{bundles}'")
            } else {
                format!("bundles: {bundles}")
            };
            assert!(
                workflow
                    .lines()
                    .any(|line| line.contains(&format!("os: {workflow_os},"))
                        && line.contains(&quoted)),
                "{os}: {quoted}"
            );
        }
        assert!(default_bundles("freebsd").is_err());
    }

    #[test]
    fn windows_runs_the_npm_batch_script() {
        assert_eq!(npm_program("windows"), "npm.cmd");
        assert_eq!(npm_program("linux"), "npm");
        assert_eq!(npm_program("macos"), "npm");
    }

    #[test]
    fn updater_artifacts_need_a_signing_key_or_an_explicit_request() {
        assert!(!creates_updater_artifacts(false, false));
        assert!(creates_updater_artifacts(false, true));
        assert!(creates_updater_artifacts(true, true));
        let config: serde_json::Value = serde_json::from_str(UNSIGNED_CONFIG).unwrap();
        assert_eq!(config["bundle"]["createUpdaterArtifacts"], false);
    }

    #[test]
    fn tauri_builds_the_requested_bundles_with_the_locked_dependencies() {
        let config = Path::new("target/xtask/build-installer.tauri.json");
        assert_eq!(
            tauri_arguments("nsis", Some(config)),
            [
                "run",
                "tauri",
                "--",
                "build",
                "--bundles",
                "nsis",
                "--config",
                "target/xtask/build-installer.tauri.json",
                "--",
                "--locked",
            ]
            .map(OsString::from)
        );
        assert_eq!(
            tauri_arguments("deb,rpm", None),
            [
                "run",
                "tauri",
                "--",
                "build",
                "--bundles",
                "deb,rpm",
                "--",
                "--locked"
            ]
            .map(OsString::from)
        );
        assert_eq!(bundle_directory("app"), "macos");
        assert_eq!(bundle_directory("nsis"), "nsis");
    }

    #[test]
    fn only_packages_from_this_build_are_reported() {
        let root =
            std::env::temp_dir().join(format!("oflh-xtask-installer-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("deb/OFLH Desktop_0.7.0_amd64/data")).unwrap();
        fs::create_dir_all(root.join("macos/OFLH Desktop.app/Contents")).unwrap();
        fs::write(root.join("deb/old.deb"), "old").unwrap();
        let started = SystemTime::now() + std::time::Duration::from_secs(1);
        assert!(new_packages(&root, "deb,rpm", started).unwrap().is_empty());
        let packages = new_packages(&root, "deb,rpm,app", SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            packages,
            [
                root.join("deb/old.deb"),
                root.join("macos/OFLH Desktop.app")
            ]
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
