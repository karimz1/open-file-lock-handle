//! Assemble public downloads; validation receipts remain internal CI artifacts.
use super::{Result, TARGETS, desktop, digest, name};
use std::{fs, io::Write, path::Path};

const RELEASE_BASE: &str = "https://github.com/karimz1/open-file-lock-handle/releases";

/// Maps a packaging target to the Tauri updater's platform key
/// (`OS-ARCH`, using the updater's own architecture names), or `None` when
/// the target has no self-update support (Linux ships no AppImage).
fn updater_platform(os: &str, arch: &str) -> Option<&'static str> {
    let arch = match arch {
        "amd64" => "x86_64",
        "arm64" => "aarch64",
        _ => return None,
    };
    match (os, arch) {
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("windows", "aarch64") => Some("windows-aarch64"),
        ("darwin", "x86_64") => Some("darwin-x86_64"),
        ("darwin", "aarch64") => Some("darwin-aarch64"),
        _ => None,
    }
}

/// Builds the static `latest.json` the updater plugin polls on GitHub
/// Releases. Reads signatures from artifacts already copied into `output`.
fn latest_manifest(output: &Path, tag: &str) -> Result<String> {
    let mut platforms = String::new();
    for (os, arch) in TARGETS {
        let (Some(platform), Some(payload_extension)) = (
            updater_platform(os, arch),
            desktop::updater_payload_extension(os),
        ) else {
            continue;
        };
        let payload_name = desktop::artifact(os, arch, payload_extension);
        let signature_name = desktop::artifact(os, arch, "sig");
        let signature = fs::read_to_string(output.join(&signature_name))?;
        if !platforms.is_empty() {
            platforms.push(',');
        }
        platforms.push_str(&format!(
            "\n    \"{platform}\": {{\n      \"url\": \"{RELEASE_BASE}/download/{tag}/{payload_name}\",\n      \"signature\": \"{}\"\n    }}",
            signature.trim()
        ));
    }
    Ok(format!(
        "{{\n  \"version\": \"{}\",\n  \"notes\": \"See {RELEASE_BASE}/tag/{tag}\",\n  \"platforms\": {{{platforms}\n  }}\n}}\n",
        &tag[1..]
    ))
}

pub(super) fn assemble(cli: &Path, desktop_dir: &Path, output: &Path, tag: &str) -> Result<()> {
    // Validate both complete native matrices before copying anything public.
    super::assemble(cli, tag, None)?;
    desktop::assemble(desktop_dir, tag)?;
    if output.exists() {
        return Err("refusing to overwrite a release directory".into());
    }
    let mut sources = Vec::new();
    for (os, arch) in TARGETS {
        sources.push(cli.join(name(os, arch)?));
        for extension in desktop::extensions(os)? {
            sources.push(desktop_dir.join(desktop::artifact(os, arch, extension)));
        }
    }
    sources.sort();
    fs::create_dir_all(output)?;
    let mut checksums = fs::File::create(output.join("checksums.txt"))?;
    for source in sources {
        let filename = source.file_name().ok_or("missing artifact filename")?;
        let destination = output.join(filename);
        fs::copy(&source, &destination)?;
        writeln!(
            checksums,
            "{}  {}",
            digest(&destination)?,
            filename.to_string_lossy()
        )?;
    }
    fs::write(output.join("latest.json"), latest_manifest(output, tag)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_release_has_direct_downloads_and_one_checksum_file() {
        let root = std::env::temp_dir().join(format!("oflh-public-release-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("binary");
        fs::write(&binary, b"tested fixture").unwrap();
        let cli = root.join("cli");
        let desktop_dir = root.join("desktop");
        let output = root.join("public");
        for (os, arch) in TARGETS {
            super::super::package(&binary, &cli, "v1.2.3", os, arch).unwrap();
            let bundle = root.join(format!("{os}-{arch}"));
            fs::create_dir_all(&bundle).unwrap();
            for extension in desktop::extensions(os).unwrap() {
                fs::write(
                    bundle.join(format!("fixture.{extension}")),
                    format!("installer-{os}-{arch}-{extension}").as_bytes(),
                )
                .unwrap();
            }
            desktop::package(&bundle, &binary, &desktop_dir, "v1.2.3", os, arch).unwrap();
        }
        assemble(&cli, &desktop_dir, &output, "v1.2.3").unwrap();
        let sums = fs::read_to_string(output.join("checksums.txt")).unwrap();
        assert_eq!(sums.lines().count(), 22);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 24);
        assert!(sums.contains("oflh-cli.windows.arm64.exe"));
        assert!(sums.contains("oflh-desktop.linux.amd64.rpm"));
        assert!(sums.contains("oflh-desktop.linux.amd64.tar.gz"));
        assert!(sums.contains("oflh-desktop.linux.arm64.tar.gz"));
        assert!(sums.contains("oflh-desktop.windows.amd64-installer.exe"));
        assert!(sums.contains("oflh-desktop.windows.amd64-installer.exe.sig"));
        assert!(sums.contains("oflh-desktop.darwin.arm64.app.tar.gz"));
        assert!(sums.contains("oflh-desktop.darwin.arm64.app.tar.gz.sig"));
        assert!(!sums.contains("oflh-desktop.windows.amd64.exe"));
        assert!(!sums.contains("manifest"));
        assert!(!sums.contains("latest.json"));
        for line in sums.lines() {
            let (hash, filename) = line.split_once("  ").unwrap();
            assert_eq!(hash, digest(&output.join(filename)).unwrap());
        }
        let manifest = fs::read_to_string(output.join("latest.json")).unwrap();
        assert!(manifest.contains("\"version\": \"1.2.3\""));
        assert!(manifest.contains("\"windows-x86_64\""));
        assert!(manifest.contains("\"windows-aarch64\""));
        assert!(manifest.contains("\"darwin-x86_64\""));
        assert!(manifest.contains("\"darwin-aarch64\""));
        assert!(!manifest.contains("linux"));
        assert!(manifest.contains(
            "https://github.com/karimz1/open-file-lock-handle/releases/download/v1.2.3/oflh-desktop.windows.amd64-installer.exe"
        ));
        assert!(manifest.contains("installer-windows-amd64-sig"));
        assert!(manifest.contains("installer-darwin-arm64-sig"));
        assert!(assemble(&cli, &desktop_dir, &output, "v1.2.3").is_err());
        fs::write(
            desktop_dir.join("oflh-desktop.linux.amd64.rpm"),
            b"tampered",
        )
        .unwrap();
        assert!(assemble(&cli, &desktop_dir, &root.join("bad"), "v1.2.3").is_err());
        assert!(!root.join("bad").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
