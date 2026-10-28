//! Linux: find the `.desktop` entry that launches an executable and resolve
//! its `Icon=` through the hicolor theme and pixmaps (PNG or SVG).

use crate::Icon;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const SIZES: [&str; 7] = [
    "48x48", "64x64", "32x32", "128x128", "256x256", "scalable", "24x24",
];

/// Executable name -> `Icon=` value from every installed `.desktop` file.
#[derive(Debug, Default)]
pub struct DesktopIndex {
    roots: Vec<PathBuf>,
    by_executable: HashMap<String, String>,
}

impl DesktopIndex {
    /// Index the `applications` folders under the given XDG data roots.
    pub fn build(roots: Vec<PathBuf>) -> Self {
        let mut by_executable = HashMap::new();
        for root in &roots {
            for file in desktop_files(&root.join("applications")) {
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                let Some(entry) = parse_entry(&text) else {
                    continue;
                };
                let stem = file
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned());
                for key in entry.executables.into_iter().chain(stem) {
                    // Earlier roots (the user's own) win, as in XDG lookup.
                    by_executable
                        .entry(key)
                        .or_insert_with(|| entry.icon.clone());
                }
            }
        }
        Self {
            roots,
            by_executable,
        }
    }
    /// The icon for `executable`, if a desktop entry names one we can read.
    pub fn icon(&self, executable: &Path) -> Option<Icon> {
        let full = executable.to_string_lossy();
        let name = executable.file_name()?.to_string_lossy();
        let icon = self
            .by_executable
            .get(full.as_ref())
            .or_else(|| self.by_executable.get(name.as_ref()))?;
        self.resolve(icon)
    }
    fn resolve(&self, icon: &str) -> Option<Icon> {
        let direct = Path::new(icon);
        if direct.is_absolute() {
            return read_icon(direct);
        }
        for root in &self.roots {
            for size in SIZES {
                for extension in ["png", "svg"] {
                    let candidate = root
                        .join("icons/hicolor")
                        .join(size)
                        .join("apps")
                        .join(format!("{icon}.{extension}"));
                    if let Some(found) = read_icon(&candidate) {
                        return Some(found);
                    }
                }
            }
            for extension in ["png", "svg"] {
                if let Some(found) = read_icon(&root.join(format!("pixmaps/{icon}.{extension}"))) {
                    return Some(found);
                }
            }
        }
        None
    }
}

struct Entry {
    icon: String,
    executables: Vec<String>,
}

fn parse_entry(text: &str) -> Option<Entry> {
    let mut in_entry = false;
    let (mut icon, mut exec, mut try_exec) = (None, None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Icon=") {
            icon = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("TryExec=") {
            try_exec = Some(value.trim().to_string());
        }
    }
    let icon = icon.filter(|icon| !icon.is_empty())?;
    let mut executables = Vec::new();
    for command in [try_exec, exec.as_deref().and_then(program)]
        .into_iter()
        .flatten()
    {
        let path = Path::new(&command);
        if let Some(name) = path.file_name() {
            executables.push(name.to_string_lossy().into_owned());
        }
        if path.is_absolute() {
            executables.push(command.clone());
        }
    }
    Some(Entry { icon, executables })
}

/// The program an `Exec=` line runs, skipping `env VAR=value` prefixes.
fn program(exec: &str) -> Option<String> {
    let mut words = words(exec).into_iter();
    let mut word = words.next()?;
    if word == "env" || word.ends_with("/env") {
        word = words.find(|word| !word.contains('=') && !word.starts_with('-'))?;
    }
    Some(word)
}

/// Split an `Exec=` value into words, honouring double quotes.
fn words(exec: &str) -> Vec<String> {
    let (mut words, mut word, mut quoted, mut escaped) = (Vec::new(), String::new(), false, false);
    for character in exec.chars() {
        match character {
            _ if escaped => {
                word.push(character);
                escaped = false;
            }
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(character),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn desktop_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![(folder.to_path_buf(), 0)];
    while let Some((folder, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && depth < 2 {
                pending.push((path, depth + 1));
            } else if path.extension().is_some_and(|ext| ext == "desktop") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn read_icon(path: &Path) -> Option<Icon> {
    let mime = match path.extension()?.to_str()? {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() as usize > crate::MAX_ICON_BYTES {
        return None;
    }
    Some(Icon {
        mime,
        bytes: std::fs::read(path).ok()?,
    })
}

/// XDG data roots in lookup order, including Flatpak exports.
fn data_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut roots = Vec::new();
    match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => roots.push(PathBuf::from(dir)),
        _ => roots.extend(home.as_ref().map(|home| home.join(".local/share"))),
    }
    roots.extend(home.map(|home| home.join(".local/share/flatpak/exports/share")));
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(
        system
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from),
    );
    roots.push("/var/lib/flatpak/exports/share".into());
    roots.dedup();
    roots
}

pub(crate) fn icon(executable: &Path) -> Option<Icon> {
    static INDEX: OnceLock<DesktopIndex> = OnceLock::new();
    INDEX
        .get_or_init(|| DesktopIndex::build(data_roots()))
        .icon(executable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, text: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn finds_the_theme_icon_of_the_launching_desktop_entry() {
        let root = tempfile::tempdir().unwrap();
        write(
            &root.path().join("applications/blender.desktop"),
            b"[Desktop Entry]\nName=Blender\nExec=env LANG=C /opt/blender/blender %f\nIcon=blender\n\n[Desktop Action New]\nIcon=other\n",
        );
        write(
            &root.path().join("icons/hicolor/48x48/apps/blender.png"),
            b"png",
        );
        write(
            &root.path().join("icons/hicolor/scalable/apps/blender.svg"),
            b"<svg/>",
        );
        let index = DesktopIndex::build(vec![root.path().to_path_buf()]);
        let icon = index.icon(Path::new("/opt/blender/blender")).unwrap();
        assert_eq!(icon.mime, "image/png");
        assert_eq!(icon.bytes, b"png");
        // Matches by name too when the binary runs from another prefix.
        assert!(index.icon(Path::new("/snap/bin/blender")).is_some());
    }

    #[test]
    fn falls_back_to_scalable_pixmaps_and_absolute_icons() {
        let root = tempfile::tempdir().unwrap();
        let absolute = root.path().join("art/tool.svg");
        write(&absolute, b"<svg/>");
        write(
            &root.path().join("applications/vendor/tool.desktop"),
            format!(
                "[Desktop Entry]\nExec=\"tool\" --flag\nIcon={}\n",
                absolute.display()
            )
            .as_bytes(),
        );
        write(
            &root.path().join("applications/code.desktop"),
            b"[Desktop Entry]\nExec=/usr/share/code/code\nIcon=vscode\n",
        );
        write(&root.path().join("pixmaps/vscode.png"), b"pix");
        let index = DesktopIndex::build(vec![root.path().to_path_buf()]);
        assert_eq!(
            index.icon(Path::new("/usr/bin/tool")).unwrap().mime,
            "image/svg+xml"
        );
        assert_eq!(
            index.icon(Path::new("/usr/share/code/code")).unwrap().bytes,
            b"pix"
        );
    }

    #[test]
    fn unknown_or_unreadable_icons_use_the_fallback() {
        let root = tempfile::tempdir().unwrap();
        write(
            &root.path().join("applications/a.desktop"),
            b"[Desktop Entry]\nExec=a\nIcon=missing\n",
        );
        write(
            &root.path().join("applications/b.desktop"),
            b"[Desktop Entry]\nExec=b\n",
        );
        write(&root.path().join("icons/hicolor/48x48/apps/c.xpm"), b"x");
        write(
            &root.path().join("applications/c.desktop"),
            b"[Desktop Entry]\nExec=c\nIcon=c\n",
        );
        let index = DesktopIndex::build(vec![root.path().to_path_buf()]);
        for name in ["/bin/a", "/bin/b", "/bin/c", "/bin/none"] {
            assert!(index.icon(Path::new(name)).is_none(), "{name}");
        }
    }

    #[test]
    fn user_entries_win_over_system_entries() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        for (root, icon) in [(&user, "mine"), (&system, "theirs")] {
            write(
                &root.path().join("applications/app.desktop"),
                format!("[Desktop Entry]\nExec=app\nIcon={icon}\n").as_bytes(),
            );
            write(
                &root
                    .path()
                    .join(format!("icons/hicolor/48x48/apps/{icon}.png")),
                icon.as_bytes(),
            );
        }
        let index =
            DesktopIndex::build(vec![user.path().to_path_buf(), system.path().to_path_buf()]);
        assert_eq!(
            index.icon(Path::new("/usr/bin/app")).unwrap().bytes,
            b"mine"
        );
    }

    #[test]
    fn exec_parsing_skips_env_prefixes() {
        assert_eq!(program("env A=1 B=2 /usr/bin/x %U").unwrap(), "/usr/bin/x");
        assert_eq!(
            program("\"/opt/My App/run\" %f").unwrap(),
            "/opt/My App/run"
        );
        assert_eq!(program("firefox %u").unwrap(), "firefox");
        assert_eq!(program(""), None);
    }
}
