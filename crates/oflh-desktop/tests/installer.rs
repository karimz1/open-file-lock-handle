//! The Windows installer hooks stay consistent with the Tauri config and the app.
use std::collections::{BTreeMap, BTreeSet};

const HOOKS: &str = include_str!("../windows/installer-hooks.nsh");

fn nsis_config() -> serde_json::Value {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    config["bundle"]["windows"]["nsis"].clone()
}

/// NSIS language name to the LANGID the hooks must use before MUI_LANGUAGE.
fn language_id(name: &str) -> &'static str {
    match name {
        "English" => "1033",
        "German" => "1031",
        "SimpChinese" => "2052",
        other => panic!("add the LANGID for installer language {other}"),
    }
}

#[test]
fn tauri_bundles_the_hooks_with_english_as_the_fallback_language() {
    let nsis = nsis_config();
    assert_eq!(nsis["installerHooks"], "windows/installer-hooks.nsh");
    let languages = nsis["languages"].as_array().unwrap();
    // NSIS falls back to the first language for unmatched display languages.
    assert_eq!(languages[0], "English");
    assert!(
        nsis.get("displayLanguageSelector")
            .is_none_or(|value| value == false)
    );
}

#[test]
fn every_installer_string_is_translated_for_every_installer_language() {
    let mut strings: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for line in HOOKS.lines().filter(|line| line.starts_with("LangString ")) {
        let mut fields = line.split_whitespace().skip(1);
        let (name, id) = (fields.next().unwrap(), fields.next().unwrap());
        assert!(
            strings.entry(name).or_default().insert(id),
            "{name} {id} twice"
        );
    }
    let expected: BTreeSet<&str> = nsis_config()["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| language_id(name.as_str().unwrap()))
        .collect();
    for name in [
        "oflhContextMenuOption",
        "oflhInspectFile",
        "oflhInspectFolder",
    ] {
        assert_eq!(strings.get(name), Some(&expected), "{name}");
    }
}

#[test]
fn explorer_commands_use_the_app_flag_and_uninstall_removes_every_class() {
    let command = format!(
        "$\\\"$INSTDIR\\${{MAINBINARYNAME}}.exe$\\\" {} $\\\"",
        oflh_desktop::launch::INSPECT_FLAG
    );
    assert!(HOOKS.contains(&command), "{command}");
    let written: BTreeSet<&str> = HOOKS
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("!insertmacro OFLH_WRITE_CONTEXT_MENU_VERB \"")
        })
        .map(|rest| rest.split('"').next().unwrap())
        .collect();
    assert_eq!(
        written,
        BTreeSet::from(["*", "Directory", "Directory\\Background", "Drive"])
    );
    for class in written {
        assert!(
            HOOKS.contains(&format!(
                "DeleteRegKey SHCTX \"Software\\Classes\\{class}\\shell\\${{OFLH_CONTEXT_MENU_VERB}}\""
            )),
            "{class} is not removed"
        );
    }
    // A folder background has no selected item; Explorer only provides %V.
    assert!(HOOKS.contains(
        "OFLH_WRITE_CONTEXT_MENU_VERB \"Directory\\Background\" \"$(oflhInspectFolder)\" \"%V\""
    ));
}
