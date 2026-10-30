//! What the context menu entry shows and which command it runs.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// CLSID of the `IExplorerCommand` handler. It must match
/// `package/AppxManifest.xml`; a test checks that.
pub const HANDLER_CLSID: &str = "6F1A0C4E-3B7D-4E59-9C2A-0F1E5D8B7A31";

/// [`HANDLER_CLSID`] as a number, for building the COM `GUID`.
pub const HANDLER_CLSID_U128: u128 = 0x6F1A0C4E_3B7D_4E59_9C2A_0F1E5D8B7A31;

/// File name of the desktop app inside the install folder (Tauri `MAINBINARYNAME`).
pub const APP_EXE: &str = "oflh-desktop.exe";

/// Folder of the install directory the DLL and identity package are bundled in.
pub const SHELL_EXT_DIR: &str = "shell-ext";

/// What the user right-clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    File,
    /// A folder, a drive, or the background of an open folder.
    Folder,
}

/// Menu label in the Windows display language, matching the classic entries
/// the installer writes ("Inspect file" / "Inspect folder").
///
/// `ui_language` is a Windows LANGID; only its primary language is used, so
/// German (Austria) or Chinese (Singapore) get the German or Chinese label.
pub fn title(kind: TargetKind, ui_language: u16) -> &'static str {
    const LANG_CHINESE: u16 = 0x04;
    const LANG_GERMAN: u16 = 0x07;
    match (ui_language & 0x3ff, kind) {
        (LANG_GERMAN, TargetKind::File) => "Datei untersuchen",
        (LANG_GERMAN, TargetKind::Folder) => "Ordner untersuchen",
        (LANG_CHINESE, TargetKind::File) => "检查文件",
        (LANG_CHINESE, TargetKind::Folder) => "检查文件夹",
        (_, TargetKind::File) => "Inspect file",
        (_, TargetKind::Folder) => "Inspect folder",
    }
}

/// The desktop app next to the handler: the DLL lives in
/// `<install dir>\shell-ext\`, the app in `<install dir>\`.
pub fn app_path(handler_dll: &Path) -> Option<PathBuf> {
    let shell_ext_dir = handler_dll.parent()?;
    let install_dir = shell_ext_dir.parent()?;
    Some(install_dir.join(APP_EXE))
}

/// Arguments for the app, identical to the classic `--inspect "%1"` verb.
/// Passed as separate arguments, so no shell quoting is involved and paths
/// with spaces or a trailing backslash (`C:\`) arrive unchanged.
pub fn inspect_args(target: &Path) -> Vec<OsString> {
    vec![OsString::from("--inspect"), target.as_os_str().to_owned()]
}

/// The entry handles one target at a time, like the classic verbs
/// (`MultiSelectModel=Single`): it is hidden for anything but one item.
pub fn visible_for(selected_items: usize) -> bool {
    selected_items == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_the_primary_display_language_with_english_fallback() {
        assert_eq!(title(TargetKind::File, 0x0409), "Inspect file");
        assert_eq!(title(TargetKind::Folder, 0x0809), "Inspect folder");
        assert_eq!(title(TargetKind::File, 0x0407), "Datei untersuchen");
        assert_eq!(title(TargetKind::Folder, 0x0c07), "Ordner untersuchen");
        assert_eq!(title(TargetKind::File, 0x0804), "检查文件");
        assert_eq!(title(TargetKind::Folder, 0x1004), "检查文件夹");
        assert_eq!(title(TargetKind::File, 0x040c), "Inspect file");
    }

    #[test]
    fn labels_match_the_classic_installer_entries() {
        let hooks = include_str!("../../oflh-desktop/windows/installer-hooks.nsh");
        for (name, id, kind, lang) in [
            ("oflhInspectFile", "1033", TargetKind::File, 0x0409),
            ("oflhInspectFile", "1031", TargetKind::File, 0x0407),
            ("oflhInspectFile", "2052", TargetKind::File, 0x0804),
            ("oflhInspectFolder", "1033", TargetKind::Folder, 0x0409),
            ("oflhInspectFolder", "1031", TargetKind::Folder, 0x0407),
            ("oflhInspectFolder", "2052", TargetKind::Folder, 0x0804),
        ] {
            let line = format!("LangString {name} {id} \"{}\"", title(kind, lang));
            assert!(hooks.lines().any(|l| l == line), "missing: {line}");
        }
    }

    #[test]
    fn app_is_found_one_folder_above_the_handler() {
        let dll = Path::new("install")
            .join(SHELL_EXT_DIR)
            .join("oflh_shell_ext.dll");
        assert_eq!(app_path(&dll), Some(Path::new("install").join(APP_EXE)));
        assert_eq!(app_path(Path::new("oflh_shell_ext.dll")), None);
    }

    #[test]
    fn inspect_arguments_keep_the_path_as_one_argument() {
        let target = Path::new(r"C:\Program Files\some file.txt");
        assert_eq!(
            inspect_args(target),
            vec![
                OsString::from("--inspect"),
                OsString::from(r"C:\Program Files\some file.txt")
            ]
        );
        assert_eq!(inspect_args(Path::new(r"C:\"))[1], OsString::from(r"C:\"));
    }

    #[test]
    fn clsid_string_and_number_agree() {
        let hex: String = HANDLER_CLSID.chars().filter(|c| *c != '-').collect();
        assert_eq!(u128::from_str_radix(&hex, 16).unwrap(), HANDLER_CLSID_U128);
    }

    #[test]
    fn entry_is_shown_for_exactly_one_item() {
        assert!(!visible_for(0));
        assert!(visible_for(1));
        assert!(!visible_for(2));
    }

    #[test]
    fn manifest_declares_this_handler_for_files_folders_and_background() {
        let manifest = include_str!("../package/AppxManifest.xml");
        let clsid = format!("\"{HANDLER_CLSID}\"");
        assert!(
            manifest.matches(&clsid).count() >= 4,
            "manifest CLSID drifted"
        );
        for item_type in ["\"*\"", "\"Directory\"", "\"Directory\\Background\""] {
            assert!(
                manifest.contains(&format!("ItemType Type={item_type}")),
                "{item_type}"
            );
        }
        assert!(manifest.contains(&format!("Path=\"{SHELL_EXT_DIR}\\oflh_shell_ext.dll\"")));
        assert!(manifest.contains(&format!("Executable=\"{APP_EXE}\"")));
        // Unsigned builds need Microsoft's unsigned-publisher OID; the packaging
        // script swaps the publisher only when a signing certificate is used.
        assert!(manifest.contains("OID.2.25.311729368913984317654407730594956997722=1"));
    }
}
