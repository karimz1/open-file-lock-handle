//! Assemble public downloads; validation receipts remain internal CI artifacts.
use super::{Result, TARGETS, desktop, digest, name};
use std::{fs, io::Write, path::Path};

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
                fs::write(bundle.join(format!("fixture.{extension}")), b"installer").unwrap();
            }
            desktop::package(&bundle, &binary, &desktop_dir, "v1.2.3", os, arch).unwrap();
        }
        assemble(&cli, &desktop_dir, &output, "v1.2.3").unwrap();
        let sums = fs::read_to_string(output.join("checksums.txt")).unwrap();
        assert_eq!(sums.lines().count(), 14);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 15);
        assert!(sums.contains("oflh-cli.windows.arm64.exe"));
        assert!(sums.contains("oflh-desktop.linux.amd64.rpm"));
        assert!(!sums.contains("manifest"));
        assert!(!sums.contains(".zip"));
        for line in sums.lines() {
            let (hash, filename) = line.split_once("  ").unwrap();
            assert_eq!(hash, digest(&output.join(filename)).unwrap());
        }
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
