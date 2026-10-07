use oflh_core::*;
#[test]
fn escaping() {
    assert_eq!(safe("ok\x1b[31m\n\u{202e}"), "ok�[31m��");
}
#[test]
fn paths() {
    let root = std::env::temp_dir().join(format!("oflh-core-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let p = root.join("file ü");
    std::fs::write(&p, b"data").unwrap();
    let target = Target::new(&p).unwrap();
    assert!(!target.directory);
    assert!(target.matches(&p, std::fs::metadata(&p).ok().as_ref()));
    let dir = Target::new(&root).unwrap();
    assert!(dir.contains(&p));
    assert!(!dir.contains(&root.with_extension("other")));
    assert!(
        Target::new(root.join("missing"))
            .unwrap()
            .metadata
            .is_none()
    );
    #[cfg(unix)]
    {
        let alias = root.join("alias");
        std::fs::hard_link(&p, &alias).unwrap();
        assert!(target.matches(&alias, std::fs::metadata(&alias).ok().as_ref()));
        let link = root.join("link");
        std::os::unix::fs::symlink(&p, &link).unwrap();
        assert_eq!(Target::new(link).unwrap().path, target.path);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn native_drive_root_and_extended_root_resolve_to_the_same_target() {
    let root = Target::new(r"C:\").unwrap();
    let extended = Target::new(r"\\?\C:\").unwrap();
    assert!(root.directory);
    assert!(root.metadata.as_ref().unwrap().is_dir());
    assert_eq!(root.path, extended.path);
    assert_eq!(display_path(&root.path), r"C:\");
    let fixture = root
        .path
        .join("oflh-synthetic-missing-folder")
        .join("file.bin");
    assert!(root.contains(&fixture));
    assert!(root.contains(std::path::Path::new(
        r"c:\oflh-synthetic-missing-folder\file.bin"
    )));
    assert!(!root.contains(std::path::Path::new(
        r"D:\oflh-synthetic-missing-folder\file.bin"
    )));
    assert_eq!(clipboard_path_text(root.path.to_str().unwrap()), r"C:\");
    // Presentation never mutates the native canonical action/matching reference.
    assert_eq!(root.path, std::fs::canonicalize(r"C:\").unwrap());
}

#[cfg(windows)]
#[test]
fn display_cannot_replace_distinct_non_unicode_native_paths() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let prefix: Vec<_> = r"\\?\C:\fixture\".encode_utf16().collect();
    let make_path = |surrogate| {
        let mut units = prefix.clone();
        units.push(surrogate);
        std::path::PathBuf::from(OsString::from_wide(&units))
    };
    let first = make_path(0xd800);
    let second = make_path(0xd801);
    assert_eq!(display_path(&first), display_path(&second));
    assert_ne!(first, second);
    assert!(first.to_str().is_none());
    assert!(second.to_str().is_none());
    let root = Target::new(r"C:\").unwrap();
    assert!(root.contains(&first));
    assert!(root.contains(&second));
}
