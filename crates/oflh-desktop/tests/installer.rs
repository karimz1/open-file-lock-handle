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
        "oflhWelcomeText",
        "oflhFinishText",
        "oflhRunApp",
        "oflhDirectoryText",
        "oflhUninstallText",
        "oflhInspectFile",
        "oflhInspectFolder",
        "oflhWebView2MissingPrompt",
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

/// Width, height and bits per pixel of an uncompressed BMP.
fn bmp_info(path: &str) -> (i32, i32, u16, u32) {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    assert_eq!(&bytes[..2], b"BM", "{path} is not a BMP");
    let i32_at = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let bits = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(bytes[30..34].try_into().unwrap());
    (i32_at(18), i32_at(22).abs(), bits, compression)
}

#[test]
fn installer_uses_the_oflh_icon_and_branded_bitmaps_at_three_times_mui_sizes() {
    let nsis = nsis_config();
    let icon = nsis["installerIcon"].as_str().unwrap();
    assert!(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(icon)
            .is_file()
    );
    // MUI2 reserves 150x57 for the header and 164x314 for the sidebar; NSIS
    // scales 300% source art down rather than magnifying a 96-DPI raster.
    // Keep the aspect ratio and use reliable 24-bit uncompressed BMPs.
    for (key, size) in [
        ("headerImage", (150, 57)),
        ("uninstallerHeaderImage", (150, 57)),
        ("sidebarImage", (164, 314)),
    ] {
        let path = nsis[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} is not set"));
        assert_eq!(bmp_info(path), (size.0 * 3, size.1 * 3, 24, 0), "{path}");
    }
    // NSIS's own LoadImage shrink is nearest-neighbour; both bitmaps are
    // reshrunk with HALFTONE averaging so the logo edges stay smooth.
    assert!(HOOKS.contains("gdi32::SetStretchBltMode(p r9, i 4)"));
    assert!(HOOKS.contains("OFLH_SMOOTH_IMAGE $1 \"$PLUGINSDIR\\modern-wizard.bmp\""));
    assert!(HOOKS.contains("OFLH_SMOOTH_IMAGE $0 \"$PLUGINSDIR\\modern-header.bmp\""));
    // The uninstaller never falls back to NSIS's stock icon or wizard bitmap.
    let template = include_str!("../windows/installer.nsi");
    assert!(template.contains("!define MUI_UNICON \"${INSTALLERICON}\""));
    assert!(template.contains("!define MUI_UNWELCOMEFINISHPAGE_BITMAP \"${SIDEBARIMAGE}\""));
}

fn define(name: &str) -> u32 {
    let prefix = format!("!define {name} ");
    HOOKS
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("{name} is not defined"))
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn context_menu_checkbox_fits_the_welcome_page_and_starts_checked() {
    let text_bottom = 45 + define("OFLH_WELCOME_TEXT_HEIGHT");
    let top = define("OFLH_CHECKBOX_TOP");
    let height = define("OFLH_CHECKBOX_HEIGHT");
    // Below the shortened welcome text, room for three wrapped lines (8u each),
    // and clear of the 193u page bottom with a margin.
    assert!(top >= text_bottom + 5, "overlaps the welcome text");
    assert!(height >= 24, "too short for wrapped labels");
    assert!(top + height <= 193 - 10, "too close to the page bottom");
    // Positioned in dialog units, which scale with DPI, never in pixels.
    assert!(HOOKS.contains(
        "${NSD_CreateCheckbox} 120u ${OFLH_CHECKBOX_TOP}u 183u ${OFLH_CHECKBOX_HEIGHT}u"
    ));
    assert!(HOOKS.contains("user32::MapDialogRect"));
    // Checked unless the user unchecked it earlier in this run.
    assert!(HOOKS.contains(
        "${If} $OflhContextMenuChoice != \"0\"\n    ${NSD_Check} $OflhContextMenuCheckbox"
    ));
    assert!(HOOKS.contains("StrCpy $OflhContextMenuChoice \"1\"\n    ${Else}"));
}

#[test]
fn welcome_page_uses_the_oflh_text_instead_of_the_close_other_apps_advice() {
    assert!(HOOKS.contains("!define MUI_WELCOMEPAGE_TEXT \"$(oflhWelcomeText)\""));
    assert!(!HOOKS.contains("TEXT_WELCOME_INFO_TEXT"));
    let welcome: Vec<&str> = HOOKS
        .lines()
        .filter(|line| line.starts_with("LangString oflhWelcomeText "))
        .collect();
    assert_eq!(welcome.len(), 3);
    for line in welcome {
        assert!(line.contains("OFLH Desktop"), "{line}");
        let lower = line.to_lowercase();
        for stock in [
            "close all other applications",
            "anderen programme",
            "reboot",
            "neustart",
        ] {
            assert!(!lower.contains(stock), "{line}");
        }
    }
}

fn bundle_config() -> serde_json::Value {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    config["bundle"].clone()
}

#[test]
fn installer_metadata_names_the_publisher_website_and_cargo_version() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    // Without a version here Tauri uses the Cargo package version, which
    // ends up in DisplayVersion and the setup exe's ProductVersion.
    assert!(config.get("version").is_none());
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    let bundle = bundle_config();
    // Publisher also names the registry keys of existing installs; keep it.
    assert_eq!(bundle["publisher"], "Karim Zouine");
    assert_eq!(bundle["homepage"], "https://oflh.karimzouine.com/");
    assert!(
        bundle["copyright"]
            .as_str()
            .unwrap()
            .contains("Karim Zouine")
    );
    assert!(HOOKS.contains("!define OFLH_WEBSITE_LABEL \"oflh.karimzouine.com\""));
    // The website is in the branding line on every page, not a finish page link.
    assert!(!HOOKS.contains("MUI_FINISHPAGE_LINK"));
    assert!(HOOKS.contains("${OFLH_WEBSITE_LABEL}\""));
    assert!(HOOKS.contains("GetDlgItem $0 $HWNDPARENT 1028"));
    // The branding line reads the version from the setup exe, not a literal.
    assert!(HOOKS.contains("GetDLLVersion \"$EXEPATH\""));
    assert!(!HOOKS.contains(env!("CARGO_PKG_VERSION")));
}

const LANGUAGE_FILES: [(&str, &str, &str); 3] = [
    (
        "English",
        "ENGLISH",
        include_str!("../windows/languages/English.nsh"),
    ),
    (
        "German",
        "GERMAN",
        include_str!("../windows/languages/German.nsh"),
    ),
    (
        "SimpChinese",
        "SIMPCHINESE",
        include_str!("../windows/languages/SimpChinese.nsh"),
    ),
];

/// Every message key of tauri-bundler's installer template.
const TAURI_MESSAGES: [&str; 27] = [
    "addOrReinstall",
    "alreadyInstalled",
    "alreadyInstalledLong",
    "appRunning",
    "appRunningOkKill",
    "chooseMaintenanceOption",
    "chooseHowToInstall",
    "createDesktop",
    "dontUninstall",
    "dontUninstallDowngrade",
    "failedToKillApp",
    "installingWebview2",
    "newerVersionInstalled",
    "older",
    "olderOrUnknownVersionInstalled",
    "silentDowngrades",
    "unableToUninstall",
    "uninstallApp",
    "uninstallBeforeInstalling",
    "unknown",
    "webview2AbortError",
    "webview2DownloadError",
    "webview2DownloadSuccess",
    "webview2Downloading",
    "webview2InstallError",
    "webview2InstallSuccess",
    "deleteAppData",
];

#[test]
fn custom_installer_messages_cover_every_tauri_key_in_every_language() {
    let files = &nsis_config()["customLanguageFiles"];
    for (language, id, text) in LANGUAGE_FILES {
        assert_eq!(files[language], format!("windows/languages/{language}.nsh"));
        let mut keys = BTreeSet::new();
        for line in text.lines().filter(|line| line.starts_with("LangString ")) {
            let mut fields = line.split_whitespace().skip(1);
            let key = fields.next().unwrap();
            assert_eq!(
                fields.next(),
                Some(format!("${{LANG_{id}}}").as_str()),
                "{line}"
            );
            assert!(keys.insert(key), "{language}: {key} twice");
        }
        let tauri: BTreeSet<&str> = keys
            .iter()
            .copied()
            .filter(|key| !key.starts_with("MUI_"))
            .collect();
        assert_eq!(tauri, BTreeSet::from(TAURI_MESSAGES), "{language}");
        // Custom files are not run through Tauri's template engine.
        assert!(!text.contains("{{"), "{language}");
        assert!(!text.contains(" kill"), "{language}");
    }
    // German matches the app's informal tone, in Tauri's messages, the MUI
    // page texts replaced there, and the hook strings.
    let german = LANGUAGE_FILES[1].2;
    let hook_german = HOOKS
        .lines()
        .filter(|line| line.starts_with("LangString ") && line.contains(" 1031 "));
    for line in german
        .lines()
        .filter(|line| line.starts_with("LangString "))
        .chain(hook_german)
    {
        let words: Vec<&str> = line
            .split(|c: char| !c.is_alphabetic())
            .filter(|word| !word.is_empty())
            .collect();
        for formal in ["Sie", "Ihr", "Ihre", "Ihrem", "Ihren", "Ihnen"] {
            assert!(!words.contains(&formal), "{line}");
        }
    }
    // Every visible MUI page text that is formal in NSIS's German file.
    for key in [
        "MUI_TEXT_DIRECTORY_SUBTITLE",
        "MUI_TEXT_INSTALLING_SUBTITLE",
        "MUI_UNTEXT_CONFIRM_SUBTITLE",
        "MUI_UNTEXT_UNINSTALLING_SUBTITLE",
        "MUI_TEXT_ABORTWARNING",
        "MUI_UNTEXT_ABORTWARNING",
    ] {
        assert!(
            german.contains(&format!("LangString {key} ${{LANG_GERMAN}} ")),
            "{key}"
        );
    }
    for define in ["MUI_DIRECTORYPAGE_TEXT_TOP", "MUI_UNCONFIRMPAGE_TEXT_TOP"] {
        assert!(
            HOOKS.contains(&format!("!define {define} \"$(")),
            "{define}"
        );
    }
    assert!(german.contains("\"Auch Einstellungen und App-Daten löschen\""));
    assert!(german.contains("\"${PRODUCTNAME} läuft noch. Schließe es und versuche es erneut.\""));
}

/// NSIS's `set_language` (Source/exehead/Ui.c) without a language selector:
/// an exact match on the Windows display language, then the same primary
/// language (low 10 bits), then the first language Tauri inserts, which is
/// the first entry of `bundle.windows.nsis.languages`.
fn installer_language(display_language: u16) -> &'static str {
    let nsis = nsis_config();
    let ids: Vec<(u16, &'static str)> = nsis["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| {
            let id = language_id(name.as_str().unwrap());
            (id.parse().unwrap(), id)
        })
        .collect();
    for mask in [u16::MAX, 0x3ff, 0] {
        if let Some((_, id)) = ids
            .iter()
            .find(|(id, _)| (display_language ^ id) & mask == 0)
        {
            return id;
        }
    }
    unreachable!()
}

#[test]
fn installer_language_follows_windows_with_english_as_the_fallback() {
    let languages = nsis_config()["languages"].clone();
    assert_eq!(
        languages,
        serde_json::json!(["English", "German", "SimpChinese"])
    );
    for (display_language, expected) in [
        (0x0409, "1033"), // en-US
        (0x0809, "1033"), // en-GB
        (0x0407, "1031"), // de-DE
        (0x0C07, "1031"), // de-AT
        (0x0807, "1031"), // de-CH
        (0x0804, "2052"), // zh-CN
        (0x1004, "2052"), // zh-SG
        // Traditional Chinese shares the primary language and gets the
        // Simplified Chinese installer, as NSIS has no closer table here.
        (0x0404, "2052"), // zh-TW
        (0x0C04, "2052"), // zh-HK
        (0x0401, "1033"), // ar-SA
        (0x040C, "1033"), // fr-FR
        (0x0411, "1033"), // ja-JP
    ] {
        assert_eq!(
            installer_language(display_language),
            expected,
            "{display_language:#06x}"
        );
    }
}

#[test]
fn running_app_dialog_is_owned_centered_and_cancelled_by_default() {
    assert!(HOOKS.contains("!macroundef CheckIfAppIsRunning"));
    assert!(HOOKS.contains("comctl32::TaskDialogIndirect"));
    assert!(HOOKS.contains("p $HWNDPARENT, p 0, i 0x1008, i 9"));
    assert!(HOOKS.contains("p r3, i 0, p 0, i 2"));
    // MAKEINTRESOURCEW(-1) zero-extends a WORD. A sign-extended pointer is
    // interpreted as an invalid string address instead of a stock icon.
    assert!(HOOKS.contains("i 9, p r1, p 65535, p r2"));
    assert!(HOOKS.contains("StrCpy $R4 2"));
    assert!(HOOKS.contains("IfSilent kill_${UniqueID} 0"));
    assert!(HOOKS.contains("$PassiveMode != 1"));
    assert!(HOOKS.contains("RestartManager_EndSession $R0"));
}

#[test]
fn webview_install_mode_embeds_the_bootstrapper() {
    let mode = bundle_config()["windows"]["webviewInstallMode"].clone();
    assert_eq!(
        mode,
        serde_json::json!({ "type": "embedBootstrapper", "silent": true })
    );
    let template = include_str!("../windows/installer.nsi");
    // Custom template must keep Tauri's WebView2 section and mode placeholders.
    assert!(template.contains("Section WebView2"));
    assert!(template.contains("!define INSTALLWEBVIEW2MODE \"{{install_webview2_mode}}\""));
    assert!(
        template.contains("!define WEBVIEW2BOOTSTRAPPERPATH \"{{webview2_bootstrapper_path}}\"")
    );
    assert!(template.contains(
        "File \"/oname=$TEMP\\MicrosoftEdgeWebview2Setup.exe\" \"${WEBVIEW2BOOTSTRAPPERPATH}\""
    ));
    assert!(template.contains("!if \"${INSTALLWEBVIEW2MODE}\" == \"embedBootstrapper\""));
}

#[test]
fn webview2_failure_offers_the_microsoft_download_page() {
    const DOWNLOAD_PAGE: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";
    assert!(HOOKS.contains(&format!(
        "!define OFLH_WEBVIEW2_DOWNLOAD_URL \"{DOWNLOAD_PAGE}\""
    )));
    assert!(HOOKS.contains("Function OflhWebView2InstallFailed"));
    assert!(HOOKS.contains("IfSilent oflh_wv2_abort"));
    assert!(HOOKS.contains(
        "MessageBox MB_ICONEXCLAMATION|MB_YESNO \"$(oflhWebView2MissingPrompt)\" IDNO oflh_wv2_abort"
    ));
    assert!(HOOKS.contains("ExecShell \"open\" \"${OFLH_WEBVIEW2_DOWNLOAD_URL}\""));
    assert!(HOOKS.contains("Abort \"$(webview2AbortError)\""));
    let template = include_str!("../windows/installer.nsi");
    assert!(template.contains("Call OflhWebView2InstallFailed"));
    assert!(!template.contains("Abort \"$(webview2AbortError)\""));
    for (_, _, lang) in LANGUAGE_FILES {
        assert!(
            lang.contains(DOWNLOAD_PAGE),
            "language file must mention the WebView2 download page"
        );
        assert!(
            lang.lines().any(|line| {
                line.starts_with("LangString webview2AbortError ")
                    && line.contains("${PRODUCTNAME}")
            }),
            "webview2AbortError must name the product"
        );
    }
    for line in HOOKS
        .lines()
        .filter(|line| line.starts_with("LangString oflhWebView2MissingPrompt "))
    {
        assert!(line.contains(DOWNLOAD_PAGE), "{line}");
    }
}

#[test]
fn presentation_template_tracks_the_pinned_tauri_cli_and_native_branding() {
    let nsis = nsis_config();
    assert_eq!(nsis["template"], "windows/installer.nsi");
    let template = include_str!("../windows/installer.nsi");
    let lock: serde_json::Value =
        serde_json::from_str(include_str!("../ui/package-lock.json")).unwrap();
    let cli = lock["packages"]["node_modules/@tauri-apps/cli"]["version"]
        .as_str()
        .unwrap();
    assert!(
        template.contains(&format!("Based on Tauri CLI {cli},")),
        "review the upstream template after a Tauri CLI upgrade"
    );
    // `SetFont` is the NSIS attribute; a bare `Font` line aborts makensis.
    assert!(template.contains("\nSetFont \"Segoe UI\" 9\n"));
    assert!(
        !template
            .lines()
            .any(|line| line.trim_start().starts_with("Font "))
    );
    assert!(template.contains("MUI_PAGE_CUSTOMFUNCTION_SHOW OflhDirectoryShow"));
    assert!(HOOKS.contains("CreateFont $OflhBrandTitleFont \"Segoe UI\" 24 700"));
    assert!(
        include_str!("../windows/TAURI_TEMPLATE_LICENSE").contains("Permission is hereby granted")
    );
}
