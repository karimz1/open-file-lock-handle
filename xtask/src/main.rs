//! Developer/release executable. Never linked into the installed application.
#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
mod desktop;
mod inspection;
mod release;
mod signing;
use semver::{BuildMetadata, Prerelease, Version};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const TARGETS: [(&str, &str); 6] = [
    ("linux", "amd64"),
    ("linux", "arm64"),
    ("darwin", "amd64"),
    ("darwin", "arm64"),
    ("windows", "amd64"),
    ("windows", "arm64"),
];
fn version(value: &str) -> Result<()> {
    if value == "dev" {
        return Ok(());
    }
    let Some(version_text) = value.strip_prefix('v') else {
        return Err("version must be dev or v-prefixed semantic version".into());
    };
    semver::Version::parse(version_text)?;
    Ok(())
}
fn validate_workspace_version(release_tag: &str) -> Result<()> {
    version(release_tag)?;
    if release_tag == "dev" {
        return Ok(());
    }
    let release_version = Version::parse(
        release_tag
            .strip_prefix('v')
            .ok_or("release version must start with v")?,
    )?;
    let workspace_version = Version::parse(env!("CARGO_PKG_VERSION"))?;
    if release_version != workspace_version {
        return Err(format!(
            "release tag {release_tag} does not match workspace version {workspace_version}"
        )
        .into());
    }
    Ok(())
}
fn ci_version(
    release_tag: &str,
    base_version: &str,
    pr_number: Option<&str>,
    run_id: &str,
    run_attempt: &str,
    commit: &str,
) -> Result<String> {
    if release_tag != "dev" {
        return Ok(Version::parse(
            release_tag
                .strip_prefix('v')
                .ok_or("release version must start with v")?,
        )?
        .to_string());
    }
    let run_id = run_id.parse::<u64>()?;
    let run_attempt = run_attempt.parse::<u32>()?;
    if commit.len() < 12 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("commit must be a full hexadecimal SHA".into());
    }
    if pr_number.is_some_and(|number| {
        number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return Err("pull request number must contain only digits".into());
    }
    let mut version = Version::parse(base_version)?;
    let prerelease = pr_number.map_or_else(|| "dev".to_owned(), |number| format!("dev.pr{number}"));
    version.pre = Prerelease::new(&prerelease)?;
    version.build = BuildMetadata::new(&format!(
        "run.{run_id}.attempt.{run_attempt}.sha.{}",
        &commit[..12]
    ))?;
    Ok(version.to_string())
}
fn append_github_env(path: &Path, entries: &[(&str, &str)]) -> Result<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    for (name, value) in entries {
        if value.contains(['\n', '\r']) {
            return Err("build metadata cannot contain newlines".into());
        }
        writeln!(file, "{name}={value}")?;
    }
    Ok(())
}
fn name(os: &str, arch: &str) -> Result<String> {
    if !TARGETS.contains(&(os, arch)) {
        return Err("unsupported target".into());
    }
    Ok(format!(
        "oflh-cli.{os}.{arch}{}",
        if os == "windows" { ".exe" } else { "" }
    ))
}
fn digest(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let bytes_read = input.read(&mut buf)?;
        if bytes_read == 0 {
            break;
        }
        hash.update(&buf[..bytes_read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn package(binary: &Path, output: &Path, tag: &str, os: &str, arch: &str) -> Result<PathBuf> {
    version(tag)?;
    let name = name(os, arch)?;
    if !binary.is_file() || binary.metadata()?.len() == 0 {
        return Err("empty or missing executable".into());
    }
    fs::create_dir_all(output)?;
    let destination = output.join(name);
    fs::copy(binary, &destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o755))?;
    }
    Ok(destination)
}
fn formula(tag: &str, sums: &BTreeMap<String, String>, base: Option<&str>) -> Result<String> {
    version(tag)?;
    if tag == "dev" {
        return Err("Homebrew requires a tagged release".into());
    }
    let base = base.map(str::to_owned).unwrap_or_else(|| {
        format!("https://github.com/karimz1/open-file-lock-handle/releases/download/{tag}")
    });
    if base.contains(['"', '\n', '\r', '\\']) {
        return Err("invalid base URL".into());
    }
    let mut out = format!(
        "class Oflh < Formula\n  desc \"Find processes using files, directories, and open handles\"\n  homepage \"https://github.com/karimz1/open-file-lock-handle\"\n  version \"{}\"\n  license \"MIT\"\n\n",
        &tag[1..]
    );
    for (os, block) in [("darwin", "macos"), ("linux", "linux")] {
        out.push_str(&format!("  on_{block} do\n"));
        for (arch, brewarch) in [("arm64", "arm"), ("amd64", "intel")] {
            let name = name(os, arch)?;
            let sha = sums.get(&name).ok_or("missing artifact checksum")?;
            if sha.len() != 64
                || !sha
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            {
                return Err("invalid checksum".into());
            }
            out.push_str(&format!("    on_{brewarch} do\n      url \"{base}/{name}\"\n      sha256 \"{sha}\"\n    end\n"));
        }
        out.push_str("  end\n\n");
    }
    out.push_str("  def install\n    bin.install Dir[\"oflh-cli.*\"][0] => \"oflh\"\n  end\n\n  test do\n    assert_match \"oflh #{version}\", shell_output(\"#{bin}/oflh --version\")\n  end\nend\n");
    Ok(out)
}
fn assemble(output: &Path, tag: &str, base: Option<&str>) -> Result<String> {
    version(tag)?;
    let expected: Vec<_> = TARGETS
        .iter()
        .map(|(os, arch)| name(os, arch))
        .collect::<Result<_>>()?;
    for entry in fs::read_dir(output)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != "checksums.txt" && !expected.contains(&name) {
            return Err(format!("unexpected release artifact: {name}").into());
        }
    }
    let mut sums = BTreeMap::new();
    for name in expected {
        let artifact_path = output.join(&name);
        if !artifact_path.is_file() || artifact_path.metadata()?.len() == 0 {
            return Err(format!("missing tested artifact: {name}").into());
        }
        sums.insert(name, digest(&artifact_path)?);
    }
    let formula = formula(tag, &sums, base)?;
    let mut checksum = File::create(output.join("checksums.txt"))?;
    for (name, digest) in &sums {
        writeln!(checksum, "{digest}  {name}")?;
    }
    Ok(formula)
}
/// Run the repository's formatting, lint, and behavior gates.
fn check() -> Result<()> {
    for arguments in [
        &["fmt", "--all", "--check"][..],
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ][..],
        &["test", "--workspace", "--locked"][..],
    ] {
        let status = std::process::Command::new("cargo")
            .args(arguments)
            .status()?;
        if !status.success() {
            return Err(format!("cargo {} failed: {status}", arguments.join(" ")).into());
        }
    }
    Ok(())
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let action = args
        .next()
        .ok_or("usage: cargo xtask check|validate|package|assemble --version TAG [options]")?;
    if action == "check" {
        if args.next().is_some() {
            return Err("check takes no options".into());
        }
        return check();
    }
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--version",
            "--os",
            "--arch",
            "--binary",
            "--output",
            "--formula",
            "--base-url",
            "--bundle-dir",
            "--cli-dir",
            "--desktop-dir",
            "--commit",
            "--run-id",
            "--run-attempt",
            "--repository",
            "--server-url",
            "--pr-number",
            "--github-env",
            "--config",
            "--payload",
            "--signature",
            "--input",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unknown option {key}").into());
        }
        let value = args.next().ok_or("option requires a value")?;
        if options.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    let required = |name: &str| {
        options
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing {name}"))
    };
    if action == "inspection-summary" {
        return inspection::write_summary(
            Path::new(required("--input")?),
            Path::new(required("--output")?),
        );
    }
    let tag = if action == "verify-updater-signature" {
        "dev"
    } else {
        required("--version")?
    };
    version(tag)?;
    let output = Path::new(options.get("--output").map_or("dist", String::as_str));
    match action.as_str() {
        "verify-updater-signature" => signing::verify(
            Path::new(required("--config")?),
            Path::new(required("--payload")?),
            Path::new(required("--signature")?),
        )?,
        "validate" => validate_workspace_version(tag)?,
        "ci-version" => {
            validate_workspace_version(tag)?;
            let commit = required("--commit")?;
            let run_id = required("--run-id")?;
            let run_attempt = required("--run-attempt")?;
            let repository = required("--repository")?;
            let server_url = required("--server-url")?;
            let pr_number = options
                .get("--pr-number")
                .map(String::as_str)
                .filter(|value| !value.is_empty());
            let build_version = ci_version(
                tag,
                env!("CARGO_PKG_VERSION"),
                pr_number,
                run_id,
                run_attempt,
                commit,
            )?;
            let build_url = format!("{server_url}/{repository}/actions/runs/{run_id}");
            let pr_url = pr_number.map_or_else(String::new, |number| {
                format!("{server_url}/{repository}/pull/{number}")
            });
            append_github_env(
                Path::new(required("--github-env")?),
                &[
                    ("OFLH_VERSION", &build_version),
                    ("OFLH_BUILD_COMMIT", commit),
                    ("OFLH_BUILD_URL", &build_url),
                    ("OFLH_PULL_REQUEST_URL", &pr_url),
                ],
            )?;
            println!("{build_version}");
        }
        "package" => {
            package(
                Path::new(required("--binary")?),
                output,
                tag,
                required("--os")?,
                required("--arch")?,
            )?;
        }
        "package-desktop" => desktop::package(
            Path::new(required("--bundle-dir")?),
            Path::new(required("--binary")?),
            output,
            tag,
            required("--os")?,
            required("--arch")?,
        )?,
        "assemble-release" => release::assemble(
            Path::new(required("--cli-dir")?),
            Path::new(required("--desktop-dir")?),
            output,
            tag,
        )?,
        "assemble-desktop" => desktop::assemble(output, tag)?,
        "assemble" => {
            let formula_text =
                assemble(output, tag, options.get("--base-url").map(String::as_str))?;
            if let Some(path) = options.get("--formula") {
                fs::write(path, formula_text)?
            } else {
                print!("{formula_text}")
            }
        }
        _ => return Err("unknown action".into()),
    }
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("release: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        for version_text in ["dev", "v0.0.10-rc.1", "v1.2.3"] {
            assert!(version(version_text).is_ok())
        }
        for version_text in ["1.2.3", "v1.2", "v1.2.3-", "v01.2.3", "v1.2.3\n"] {
            assert!(version(version_text).is_err())
        }
    }
    #[test]
    fn release_tags_must_match_the_workspace_version() {
        let workspace_version = env!("CARGO_PKG_VERSION");
        assert!(validate_workspace_version("dev").is_ok());
        assert!(validate_workspace_version(&format!("v{workspace_version}")).is_ok());
        assert!(validate_workspace_version("v99.99.99").is_err());
    }
    #[test]
    fn ci_versions_include_traceable_pr_metadata_and_preserve_release_semver() {
        assert_eq!(
            ci_version(
                "dev",
                "0.0.10-rc.1",
                Some("42"),
                "1234567890",
                "2",
                "0123456789abcdef0123456789abcdef01234567",
            )
            .unwrap(),
            "0.0.10-dev.pr42+run.1234567890.attempt.2.sha.0123456789ab"
        );
        assert_eq!(
            ci_version(
                "dev",
                "0.0.10-rc.1",
                None,
                "1234567890",
                "1",
                "0123456789abcdef0123456789abcdef01234567",
            )
            .unwrap(),
            "0.0.10-dev+run.1234567890.attempt.1.sha.0123456789ab"
        );
        assert_eq!(
            ci_version(
                "v1.2.3-rc.1",
                "0.0.10-rc.1",
                None,
                "1234567890",
                "1",
                "0123456789abcdef0123456789abcdef01234567",
            )
            .unwrap(),
            "1.2.3-rc.1"
        );
        assert!(ci_version("dev", "0.0.10", None, "id", "1", "not-a-sha").is_err());
    }
    #[test]
    fn artifacts() {
        let root = std::env::temp_dir().join(format!("oflh-package-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("binary");
        fs::write(&source, b"test payload").unwrap();
        let output = root.join("dist");
        for (os, arch) in TARGETS {
            let artifact_path = package(&source, &output, "v0.0.10-rc.1", os, arch).unwrap();
            assert_eq!(fs::read(artifact_path).unwrap(), b"test payload")
        }
        let formula_text = assemble(&output, "v0.0.10-rc.1", None).unwrap();
        assert_eq!(formula_text.matches("sha256").count(), 4);
        assert!(formula_text.contains("releases/download/v0.0.10-rc.1/oflh-cli.darwin.arm64"));
        assert_eq!(
            fs::read_to_string(output.join("checksums.txt"))
                .unwrap()
                .lines()
                .count(),
            6
        );
        fs::write(output.join("unexpected"), b"bad").unwrap();
        assert!(assemble(&output, "v0.0.10-rc.1", None).is_err());
        fs::remove_file(output.join("unexpected")).unwrap();
        fs::remove_file(output.join("oflh-cli.linux.arm64")).unwrap();
        assert!(assemble(&output, "v0.0.10-rc.1", None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
