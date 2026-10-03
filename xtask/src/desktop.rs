//! Desktop package collection is separate from the six raw CLI artifacts.
use super::{Result, TARGETS, digest, version};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Expected package extensions per OS. `darwin`'s `gz`/`sig` and `windows`'s
/// `sig` are updater artifacts created alongside the manual-install package
/// because `createUpdaterArtifacts` is enabled; Linux has none since no
/// AppImage target is built, so it falls back to a manual download link.
pub(super) fn extensions(os: &str) -> Result<&'static [&'static str]> {
    match os {
        "linux" => Ok(&["deb", "rpm"]),
        "darwin" => Ok(&["dmg", "gz", "sig"]),
        "windows" => Ok(&["exe", "sig"]),
        _ => Err("unsupported desktop OS".into()),
    }
}
pub(super) fn artifact(os: &str, arch: &str, extension: &str) -> String {
    match (os, extension) {
        ("windows", "sig") => format!("oflh-desktop.windows.{arch}-installer.exe.sig"),
        ("windows", _) => format!("oflh-desktop.windows.{arch}-installer.{extension}"),
        ("darwin", "gz") => format!("oflh-desktop.darwin.{arch}.app.tar.gz"),
        ("darwin", "sig") => format!("oflh-desktop.darwin.{arch}.app.tar.gz.sig"),
        _ => format!("oflh-desktop.{os}.{arch}.{extension}"),
    }
}
/// The updater manifest's URL must point at the installable updater package
/// (the NSIS/MSI installer on Windows, the `.app.tar.gz` on macOS), never at
/// the `.sig` file or, on macOS, the human-facing `.dmg`.
pub(super) fn updater_payload_extension(os: &str) -> Option<&'static str> {
    match os {
        "windows" => Some("exe"),
        "darwin" => Some("gz"),
        _ => None,
    }
}
fn collect(
    directory: &Path,
    found: &mut BTreeMap<String, PathBuf>,
    allowed: &[&str],
) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            // The DMG contains this application bundle; it is not a second package.
            if path.extension().is_none_or(|extension| extension != "app") {
                collect(&path, found, allowed)?;
            }
        } else if entry.file_type()?.is_file() {
            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if allowed.contains(&extension)
                && found.insert(extension.into(), path.clone()).is_some()
            {
                return Err(format!("multiple desktop packages with extension {extension}").into());
            }
        }
    }
    Ok(())
}
pub(super) fn package(
    directory: &Path,
    binary: &Path,
    output: &Path,
    tag: &str,
    os: &str,
    arch: &str,
) -> Result<()> {
    version(tag)?;
    if !TARGETS.contains(&(os, arch)) {
        return Err("unsupported desktop target".into());
    }
    if !binary.is_file() || binary.metadata()?.len() == 0 {
        return Err("missing tested desktop executable".into());
    }
    let allowed = extensions(os)?;
    let mut found = BTreeMap::new();
    collect(directory, &mut found, allowed)?;
    if found.len() != allowed.len() {
        return Err("missing native desktop installer".into());
    }
    fs::create_dir_all(output)?;
    let receipt_name = format!("oflh-desktop-{os}-{arch}.manifest");
    let mut receipt = format!("version {tag}\nbinary {}\n", digest(binary)?);
    for extension in allowed {
        let source = found.get(*extension).ok_or("missing desktop installer")?;
        if source.metadata()?.len() == 0 {
            return Err("empty desktop installer".into());
        }
        let name = artifact(os, arch, extension);
        let destination = output.join(&name);
        if destination.exists() {
            return Err("refusing to overwrite a collected desktop artifact".into());
        }
        fs::copy(source, &destination)?;
        receipt.push_str(&format!("{}  {name}\n", digest(&destination)?));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join(receipt_name))?;
    file.write_all(receipt.as_bytes())?;
    Ok(())
}
pub(super) fn assemble(output: &Path, tag: &str) -> Result<()> {
    version(tag)?;
    let mut expected = BTreeMap::new();
    for (os, arch) in TARGETS {
        let receipt_name = format!("oflh-desktop-{os}-{arch}.manifest");
        let receipt = fs::read_to_string(output.join(&receipt_name))?;
        let mut lines = receipt.lines();
        if lines.next() != Some(format!("version {tag}").as_str()) {
            return Err("desktop artifact version mismatch".into());
        }
        let binary_hash = lines
            .next()
            .and_then(|line| line.strip_prefix("binary "))
            .ok_or("missing tested executable hash")?;
        if binary_hash.len() != 64 || !binary_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid tested executable hash".into());
        }
        let mut recorded = BTreeMap::new();
        for line in lines {
            let (hash, name) = line.split_once("  ").ok_or("invalid desktop manifest")?;
            if recorded.insert(name.to_owned(), hash.to_owned()).is_some() {
                return Err("duplicate desktop manifest entry".into());
            }
        }
        let allowed = extensions(os)?;
        if recorded.len() != allowed.len() {
            return Err("unexpected desktop manifest entries".into());
        }
        for extension in allowed {
            let name = artifact(os, arch, extension);
            let actual = digest(&output.join(&name))?;
            if fs::metadata(output.join(&name))?.len() == 0 || recorded.get(&name) != Some(&actual)
            {
                return Err("desktop artifact checksum mismatch".into());
            }
            expected.insert(name, actual);
        }
        expected.insert(receipt_name.clone(), digest(&output.join(receipt_name))?);
    }
    for entry in fs::read_dir(output)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != "desktop-checksums.txt" && !expected.contains_key(&name) {
            return Err(format!("unexpected desktop artifact: {name}").into());
        }
    }
    let mut checksum = fs::File::create(output.join("desktop-checksums.txt"))?;
    for (name, hash) in expected {
        writeln!(checksum, "{hash}  {name}")?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assembly_requires_six_native_sets_and_rejects_tampering_and_extras() {
        let root =
            std::env::temp_dir().join(format!("oflh-desktop-packaging-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("binary");
        fs::write(&binary, b"tested executable").unwrap();
        let output = root.join("dist");
        for (os, arch) in TARGETS {
            let bundle = root.join(format!("{os}-{arch}"));
            fs::create_dir_all(&bundle).unwrap();
            for extension in extensions(os).unwrap() {
                fs::write(
                    bundle.join(format!("installer.{extension}")),
                    b"native installer",
                )
                .unwrap();
            }
            package(&bundle, &binary, &output, "dev", os, arch).unwrap();
            assert!(package(&bundle, &binary, &output, "dev", os, arch).is_err());
        }
        assemble(&output, "dev").unwrap();
        assert!(assemble(&output, "v1.2.3").is_err());
        fs::write(output.join("unexpected"), b"bad").unwrap();
        assert!(assemble(&output, "dev").is_err());
        fs::remove_file(output.join("unexpected")).unwrap();
        fs::write(output.join(artifact("linux", "amd64", "rpm")), b"modified").unwrap();
        assert!(assemble(&output, "dev").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_desktop_artifact_name_identifies_the_installer() {
        assert_eq!(
            artifact("windows", "amd64", "exe"),
            "oflh-desktop.windows.amd64-installer.exe"
        );
    }
}
