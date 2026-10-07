//! Terminal messages. Native names, paths, search tokens and OS errors are not translated.
use std::{fmt, str::FromStr};
/// Terminal language shared by command-line options and the interactive interface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Language {
    /// English, also the fallback for unsupported system locales.
    #[default]
    English,
    /// German.
    German,
    /// Simplified Chinese.
    Chinese,
}
/// An unsupported explicit language option.
#[derive(Debug, Clone, Copy)]
pub struct LanguageError;
impl fmt::Display for LanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected en, de or zh")
    }
}
impl std::error::Error for LanguageError {}
impl FromStr for Language {
    type Err = LanguageError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "en" => Ok(Self::English),
            "de" => Ok(Self::German),
            "zh" => Ok(Self::Chinese),
            _ => Err(LanguageError),
        }
    }
}
impl Language {
    /// Resolve the primary language of a platform locale; unknown locales use English.
    pub fn resolve(locale: &str) -> Self {
        match locale
            .trim()
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "de" => Self::German,
            "zh" => Self::Chinese,
            _ => Self::English,
        }
    }
    fn environment(variables: [Option<&str>; 3]) -> Option<Self> {
        variables
            .into_iter()
            .flatten()
            .find(|value| !value.trim().is_empty())
            .map(Self::resolve)
    }
    pub(crate) fn help(self) -> &'static str {
        match self {
            Self::English => ENGLISH_HELP,
            Self::German => GERMAN_HELP,
            Self::Chinese => CHINESE_HELP,
        }
    }
    /// Follow LC_ALL, LC_MESSAGES, LANG, then the native Windows user locale.
    pub fn system() -> Self {
        let variables = ["LC_ALL", "LC_MESSAGES", "LANG"].map(|name| std::env::var(name).ok());
        if let Some(language) =
            Self::environment(variables.each_ref().map(|value| value.as_deref()))
        {
            return language;
        }
        oflh_platform::system_locale()
            .ok()
            .flatten()
            .map_or(Self::English, |locale| Self::resolve(&locale))
    }
}
macro_rules! labels {
    ($($english:literal, $german:literal, $chinese:literal;)*) => {
        #[cfg(test)]
        const LABELS: &[(&str, &str, &str)] = &[$(($english, $german, $chinese)),*];
        impl Language {
            pub(crate) fn text(self, english: &str) -> &str {
                if self == Self::English { return english; }
                match (english, self) {
                    $(($english, Self::German) => $german, ($english, Self::Chinese) => $chinese,)*
                    _ => english,
                }
            }
        }
    }
}
labels! {
    "Cancel", "Abbrechen", "取消";
    "Force kill", "Beenden erzwingen", "强制终止";
    "Terminate", "Beenden", "终止";
    "processes", "Prozesse", "进程";
    "files", "Dateien", "文件";
    "ports", "Ports", "端口";
    "indexing", "Suchindex", "建立索引";
    "Results may be incomplete · ? details", "Ergebnisse ggf. unvollständig · ? Details", "结果可能不完整 · ? 详情";
    "Enter apply · Esc cancel · ↑↓ browse", "Enter anwenden · Esc abbrechen · ↑↓ navigieren", "Enter 应用 · Esc 取消 · ↑↓ 浏览";
    " 1 Proc ", " 1 Proz ", " 1 进程 ";
    " 1 Processes ", " 1 Prozesse ", " 1 进程 ";
    " 2 Locks ", " 2 Sperren ", " 2 锁定 ";
    " 2 Locked files ", " 2 Gesperrte Dateien ", " 2 锁定文件 ";
    " 3 Ports ", " 3 Ports ", " 3 端口 ";
    "requesting termination…", "Beenden angefordert…", "正在请求终止…";
    "LIVE · every 5s", "LIVE · nach 5s", "自动 · 间隔 5 秒";
    "MANUAL · r refresh", "MANUELL · r aktualisieren", "手动 · r 刷新";
    "Search · Enter apply · Esc cancel", "Suche · Enter anwenden · Esc abbrechen", "搜索 · Enter 应用 · Esc 取消";
    "Tree · ↑↓ choose process · k stop / x force kill · Tab/← back", "Baum · ↑↓ Prozess wählen · k beenden / x erzwingen · Tab/← zurück", "进程树 · ↑↓ 选择进程 · k 终止 / x 强制终止 · Tab/← 返回";
    "Navigation · / search · Tab/→ tree", "Navigation · / suchen · Tab/→ Baum", "导航 · / 搜索 · Tab/→ 进程树";
    "THIS PATH", "DIESER PFAD", "此路径";
    "ALL PORTS", "ALLE PORTS", "所有端口";
    "name", "Name", "名称";
    "pid", "PID", "PID";
    "port", "Port", "端口";
    "match", "Treffer", "匹配";
    "PROCESS", "PROZESS", "进程";
    "USER", "BENUTZER", "用户";
    "ACCESS", "ZUGRIFF", "访问";
    "PORTS", "PORTS", "端口";
    "MATCHED PATH", "TREFFERPFAD", "匹配路径";
    "PATH", "PFAD", "路径";
    " (deleted)", " (gelöscht)", "（已删除）";
    "locked", "gesperrt", "已锁定";
    "none detected", "keine erkannt", "未检测到";
    "CPU = share of machine · RAM = RSS", "CPU = Anteil am System · RAM = RSS", "CPU = 整机占比 · RAM = RSS";
    "ANCESTRY · focused", "PROZESSBAUM · fokussiert", "进程树 · 已聚焦";
    "ANCESTRY", "PROZESSBAUM", "进程树";
    "ACTION TARGET", "AKTIONSZIEL", "操作目标";
    "PORTS · Enter to inspect", "PORTS · Enter untersuchen", "端口 · Enter 检查";
    "PORTS · Enter, then p to inspect", "PORTS · Enter, dann p untersuchen", "端口 · Enter 后按 p 检查";
    "EXECUTABLE", "PROGRAMM", "可执行文件";
    "SELECTED PATH", "GEWÄHLTER PFAD", "所选路径";
    "oflh  /  process details", "oflh  /  Prozessdetails", "oflh  /  进程详情";
    "Process exited or PID was reused. Press Esc to return.", "Prozess beendet oder PID wiederverwendet. Esc zurück.", "进程已退出或 PID 已被重用。按 Esc 返回。";
    "unavailable", "nicht verfügbar", "不可用";
    "LOCKS ONLY", "NUR SPERREN", "仅锁定";
    "ALL USAGES", "ALLE REFERENZEN", "所有引用";
    "No usage entries match this filter.", "Keine Referenzen passen zum Filter.", "没有引用符合此筛选条件。";
    "FILE", "DATEI", "文件";
    "RELATION", "BEZUG", "关系";
    "DIRECTORY", "ORDNER", "目录";
    "FORCE KILL", "BEENDEN ERZWINGEN", "强制终止";
    "Immediate termination: no cleanup. Unsaved work may be lost.", "Sofortiges Beenden ohne Aufräumen. Ungespeicherte Arbeit kann verloren gehen.", "立即终止，不执行清理。未保存的工作可能丢失。";
    "Request a normal shutdown. Unsaved work may be lost.", "Normales Beenden anfordern. Ungespeicherte Arbeit kann verloren gehen.", "请求正常退出。未保存的工作可能丢失。";
    "Target: selected ancestor. Its application and children may be affected.", "Ziel: gewählter Vorfahr. Anwendung und Kindprozesse können betroffen sein.", "目标：所选祖先进程。其应用程序和子进程可能受到影响。";
    "Affected processes (including selections hidden by filters):", "Betroffene Prozesse (einschließlich ausgefilterter Auswahl):", "受影响的进程（包括被筛选条件隐藏的选择）：";
    "LINKS", "LINKS", "链接";
    "D  Donate: https://buymeacoffee.com/karimz1", "D  Spenden: https://buymeacoffee.com/karimz1", "D  捐赠：https://buymeacoffee.com/karimz1";
    "Press uppercase R or D to open in your browser.", "Großes R oder D öffnet den Link im Browser.", "按大写 R 或 D 在浏览器中打开链接。";
    "SCAN DETAILS", "SCANDETAILS", "扫描详情";
    "Run `oflh --version` to copy the full build report.", "Mit `oflh --version` den vollständigen Buildbericht kopieren.", "运行 `oflh --version` 可复制完整构建信息。";
    "Scanning for file locks…", "Dateisperren werden gesucht…", "正在扫描文件锁…";
    "No locked files match your filter.", "Keine gesperrten Dateien passen zum Filter.", "没有锁定文件符合筛选条件。";
    "No confirmed locks found in this scan.\nVisibility depends on permissions; press r to refresh.", "Keine bestätigten Sperren in diesem Scan.\nSichtbarkeit hängt von Rechten ab; r aktualisiert.", "本次扫描未发现已确认的锁。\n可见性取决于权限；按 r 刷新。";
    "LOCKED FILE", "GESPERRTE DATEI", "锁定文件";
    "Scanning local ports…", "Lokale Ports werden gesucht…", "正在扫描本地端口…";
    "No matching port bindings.\n/ search · r refresh · s all/this path", "Keine passenden Portbindungen.\n/ suchen · r aktualisieren · s alle/dieser Pfad", "没有匹配的端口绑定。\n/ 搜索 · r 刷新 · s 所有/此路径";
    "PORT", "PORT", "端口";
    "PROTO", "PROTO", "协议";
    "NET", "NETZ", "网络";
    "ADDRESS", "ADRESSE", "地址";
    "STATE", "STATUS", "状态";
    "yes", "ja", "是";
    "oflh / process details · ports", "oflh / Prozessdetails · Ports", "oflh / 进程详情 · 端口";
    "Process exited or PID was reused. Esc back.", "Prozess beendet oder PID wiederverwendet. Esc zurück.", "进程已退出或 PID 已被重用。Esc 返回。";
    "/ Search port, process, address…", "/ Port, Prozess, Adresse suchen…", "/ 搜索端口、进程、地址…";
    "/ Search files, DLLs, paths… · * wildcard", "/ Dateien, DLLs, Pfade suchen… · * Platzhalter", "/ 搜索文件、DLL、路径… · * 通配符";
    "/ Search PID, process, path… · * wildcard", "/ PID, Prozess, Pfad suchen… · * Platzhalter", "/ 搜索 PID、进程、路径… · * 通配符";
    "Discovering processes…\nYou can keep navigating while the scan runs.", "Prozesse werden gesucht…\nNavigation bleibt während des Scans möglich.", "正在发现进程…\n扫描期间仍可导航。";
    "No processes match your filter.\nPress / to edit it or Esc to clear.", "Keine Prozesse passen zum Filter.\n/ bearbeiten oder Esc löschen.", "没有进程符合筛选条件。\n按 / 编辑或 Esc 清除。";
    "No visible processes are using this path.\nPress r to scan again. Permissions may hide usage.", "Keine sichtbaren Prozesse verwenden diesen Pfad.\nMit r erneut scannen. Rechte können Referenzen verbergen.", "没有可见进程正在使用此路径。\n按 r 重新扫描。权限可能隐藏引用。";
    "read", "lesen", "读取";
    "write", "schreiben", "写入";
    "read/write", "lesen/schr.", "读写";
    "execute", "ausführen", "执行";
    "directory", "Ordner", "目录";
    "reference", "Referenz", "引用";
    "mapped", "zugeordnet", "映射";
    "unknown", "unbekannt", "未知";
    "open", "offen", "已打开";
    "cwd", "Arbeitsordner", "工作目录";
    "executable", "Programm", "可执行文件";
    "restart manager", "Restart Manager", "Restart Manager";
    "native file user", "Dateinutzer (nativ)", "本机文件引用者";
    "LISTEN", "LAUSCHT", "侦听";
    "BOUND", "GEBUNDEN", "已绑定";
    "Cancelling inspection…", "Untersuchung wird abgebrochen…", "正在取消检查…";
    "Inspection cancelled", "Untersuchung abgebrochen", "检查已取消";
    "Enlarge the terminal to review the selected ancestor.", "Terminal vergrößern, um den gewählten Vorfahren zu prüfen.", "请扩大终端以查看所选祖先进程。";
    "This ancestor cannot be terminated: protected or identity unavailable.", "Dieser Vorfahr kann nicht beendet werden: geschützt oder Identität unbekannt.", "无法终止此祖先进程：受保护或身份不可用。";
    "Cannot terminate: selection includes a protected process or an unavailable identity.", "Beenden nicht möglich: Auswahl enthält geschützte Prozesse oder unbekannte Identitäten.", "无法终止：所选项包含受保护的进程或不可用的身份。";
    "Enlarge the terminal to review targets before confirming.", "Terminal vergrößern, um die Ziele vor der Bestätigung zu prüfen.", "请扩大终端，在确认前查看操作目标。";
    "Requesting termination…", "Beenden wird angefordert…", "正在请求终止…";
    "Tab / ←→ choose", "Tab / ←→ wählen", "Tab / ←→ 选择";
    "Enter confirm", "Enter bestätigen", "Enter 确认";
    "Esc cancel", "Esc abbrechen", "Esc 取消";
    "/ search", "/ suchen", "/ 搜索";
    "f files", "f Dateien", "f 文件";
    "r refresh", "r aktualisieren", "r 刷新";
    "a auto", "a automatisch", "a 自动";
    "k stop", "k beenden", "k 终止";
    "x force", "x erzwingen", "x 强制终止";
    "? help", "? Hilfe", "? 帮助";
    "Esc back", "Esc zurück", "Esc 返回";
    "q quit", "q beenden", "q 退出";
    "1/2/3 tabs", "1/2/3 Ansichten", "1/2/3 视图";
    "s all/this path", "s alle/dieser Pfad", "s 所有/此路径";
    "Enter inspect", "Enter untersuchen", "Enter 检查";
    "Space select", "Space auswählen", "Space 选择";
    "p ports", "p Ports", "p 端口";
    "l locks only", "l nur Sperren", "l 仅锁定";
    "↑↓ select", "↑↓ auswählen", "↑↓ 选择";
    "←→ path", "←→ Pfad", "←→ 路径";
    "D Donate", "D Spenden", "D 捐赠";
    "↑↓ scroll", "↑↓ blättern", "↑↓ 滚动";
    "↑↓ process", "↑↓ Prozess", "↑↓ 进程";
    "k stop target", "k Ziel beenden", "k 终止目标";
    "x force kill target", "x Ziel erzwingen", "x 强制终止目标";
    "Tab/← back", "Tab/← zurück", "Tab/← 返回";
    "↑↓ move", "↑↓ bewegen", "↑↓ 移动";
    "Ctrl+A all", "Ctrl+A alle", "Ctrl+A 全选";
    "Tab/→ tree", "Tab/→ Baum", "Tab/→ 进程树";
    "i panel", "i Seitenleiste", "i 侧栏";
    "m/c RAM/CPU", "m/c RAM/CPU", "m/c RAM/CPU";
    "m RAM / c CPU / n name / p PID", "m RAM / c CPU / n Name / p PID", "m RAM / c CPU / n 名称 / p PID";
    "l locks", "l Sperren", "l 锁定";
    "Tab / ←→ choose · Enter confirm · Esc cancel", "Tab / ←→ wählen · Enter bestätigen · Esc abbrechen", "Tab / ←→ 选择 · Enter 确认 · Esc 取消";
    "/ search · f files · r refresh · a auto · k stop · x force · ? help · Esc back · q quit", "/ suchen · f Dateien · r aktualisieren · a automatisch · k beenden · x erzwingen · ? Hilfe · Esc zurück · q beenden", "/ 搜索 · f 文件 · r 刷新 · a 自动 · k 终止 · x 强制终止 · ? 帮助 · Esc 返回 · q 退出";
    "1/2/3 tabs · / search · s all/this path · Enter inspect · Space select · r refresh · a auto · k stop · x force · ? help · q quit", "1/2/3 Ansichten · / suchen · s alle/dieser Pfad · Enter untersuchen · Space auswählen · r aktualisieren · a automatisch · k beenden · x erzwingen · ? Hilfe · q beenden", "1/2/3 视图 · / 搜索 · s 所有/此路径 · Enter 检查 · Space 选择 · r 刷新 · a 自动 · k 终止 · x 强制终止 · ? 帮助 · q 退出";
    "/ search · p ports · l locks only · ↑↓ select · ←→ path · r refresh · a auto · k stop · x force · R GitHub · D Donate · Esc back · q quit", "/ suchen · p Ports · l nur Sperren · ↑↓ auswählen · ←→ Pfad · r aktualisieren · a automatisch · k beenden · x erzwingen · R GitHub · D Spenden · Esc zurück · q beenden", "/ 搜索 · p 端口 · l 仅锁定 · ↑↓ 选择 · ←→ 路径 · r 刷新 · a 自动 · k 终止 · x 强制终止 · R GitHub · D 捐赠 · Esc 返回 · q 退出";
    "↑↓ scroll · R GitHub · D Donate · Esc back · q quit", "↑↓ blättern · R GitHub · D Spenden · Esc zurück · q beenden", "↑↓ 滚动 · R GitHub · D 捐赠 · Esc 返回 · q 退出";
    "↑↓ process · k stop target · x force kill target · Tab/← back", "↑↓ Prozess · k Ziel beenden · x Ziel erzwingen · Tab/← zurück", "↑↓ 进程 · k 终止目标 · x 强制终止目标 · Tab/← 返回";
    "1/2/3 tabs · / search · ↑↓ move · Enter inspect · Space select · Ctrl+A all · Tab/→ tree · i panel · m/c RAM/CPU · a auto · r refresh · k stop · x force · ? help · R GitHub · D Donate · q quit", "1/2/3 Ansichten · / suchen · ↑↓ bewegen · Enter untersuchen · Space auswählen · Ctrl+A alle · Tab/→ Baum · i Seitenleiste · m/c RAM/CPU · a automatisch · r aktualisieren · k beenden · x erzwingen · ? Hilfe · R GitHub · D Spenden · q beenden", "1/2/3 视图 · / 搜索 · ↑↓ 移动 · Enter 检查 · Space 选择 · Ctrl+A 全选 · Tab/→ 进程树 · i 侧栏 · m/c RAM/CPU · a 自动 · r 刷新 · k 终止 · x 强制终止 · ? 帮助 · R GitHub · D 捐赠 · q 退出";
    "Enter inspect · Space select · m RAM / c CPU / n name / p PID", "Enter untersuchen · Space auswählen · m RAM / c CPU / n Name / p PID", "Enter 检查 · Space 选择 · m RAM / c CPU / n 名称 / p PID";
    "/ search · r refresh · a auto · ? help · Esc back · q quit", "/ suchen · r aktualisieren · a automatisch · ? Hilfe · Esc zurück · q beenden", "/ 搜索 · r 刷新 · a 自动 · ? 帮助 · Esc 返回 · q 退出";
    "1/2/3 tabs · / search · Enter inspect · k stop · x force · ? help · R GitHub · D Donate · q quit", "1/2/3 Ansichten · / suchen · Enter untersuchen · k beenden · x erzwingen · ? Hilfe · R GitHub · D Spenden · q beenden", "1/2/3 视图 · / 搜索 · Enter 检查 · k 终止 · x 强制终止 · ? 帮助 · R GitHub · D 捐赠 · q 退出";
    "↑↓ select · / search · p ports · l locks · r refresh · ? help · Esc back · q quit", "↑↓ auswählen · / suchen · p Ports · l Sperren · r aktualisieren · ? Hilfe · Esc zurück · q beenden", "↑↓ 选择 · / 搜索 · p 端口 · l 锁定 · r 刷新 · ? 帮助 · Esc 返回 · q 退出";
}
// Literal formats compile in all languages; arguments are evaluated in only one branch.
macro_rules! localized {
    ($language:expr, "{} {phase} · {:.1}s · {} processes / {} resources · z cancel" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} {phase} · {:.1}s · {} processes / {} resources · z cancel" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("z abbrechen · {} {phase} · {:.1}s · Prozesse {} / Ressourcen {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("z 取消 · {} {phase} · {:.1}秒 · {} 个进程 / {} 个资源" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Last scan {:.2}s" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Last scan {:.2}s" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Letzter Scan {:.2}s" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("上次扫描 {:.2}秒" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "oflh {}\n{}\nResize for full view · Esc back · q quit" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("oflh {}\n{}\nResize for full view · Esc back · q quit" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("oflh {}\n{}\nFür vollständige Ansicht vergrößern · Esc zurück · q beenden" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("oflh {}\n{}\n扩大终端查看完整视图 · Esc 返回 · q 退出" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} targets. Enlarge to review." $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} targets. Enlarge to review." $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{} Ziele. Zum Prüfen vergrößern." $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} 个目标。请扩大终端查看。" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} scanning" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} scanning" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{} scannt" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} 正在扫描" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} bindings · {} · s scope" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} bindings · {} · s scope" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Bindungen: {} · {} · s Bereich" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} 个绑定 · {} · s 范围" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} locked files · {} lock entries" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} locked files · {} lock entries" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Gesperrte Dateien: {} · Sperreinträge: {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} 个锁定文件 · {} 条锁记录" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} of {} processes" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} of {} processes" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Prozesse: {} / {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} / {} 个进程" $(, $($arguments)*)?),
        }
    };
    ($language:expr, " · {} selected" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!(" · {} selected" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!(" · {} ausgewählt" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!(" · 已选 {} 项" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{summary}{} · sort: {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{summary}{} · sort: {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{summary}{} · Sortierung: {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{summary}{} · 排序：{}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, " · +{} more" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!(" · +{} more" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!(" · +{} weitere" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!(" · 另有 {} 项" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "PARENT {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("PARENT {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("ELTERNPROZESS {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("父进程 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "CPU {} machine · RAM {} RSS" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("CPU {} machine · RAM {} RSS" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("CPU {} System · RAM {} RSS" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("CPU {} 整机 · RAM {} RSS" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "CPU {} · RAM {} · PARENT {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("CPU {} · RAM {} · PARENT {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("CPU {} · RAM {} · ELTERN {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("CPU {} · RAM {} · 父进程 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "PORTS · p inspect · {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("PORTS · p inspect · {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("PORTS · p untersuchen · {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("端口 · p 检查 · {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} · {} of {} usages · {locked} locked files   {} / {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} · {} of {} usages · {locked} locked files   {} / {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{} · Referenzen: {} / {} · gesperrte Dateien: {locked}   {} / {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} · {} / {} 条引用 · {locked} 个锁定文件   {} / {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "FILE {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("FILE {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("DATEI {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("文件 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "SELECTED PATH · {} · {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("SELECTED PATH · {} · {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("GEWÄHLTER PFAD · {} · {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("所选路径 · {} · {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} {} processes?" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} {} processes?" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{} · Prozesse: {}?" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} {} 个进程？" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "VERSION {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("VERSION {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("VERSION {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("版本 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Commit {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Commit {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Commit {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("提交 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Build {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Build {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Build {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("构建 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Pull request {}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Pull request {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Pull Request {}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("拉取请求 {}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{} of {} bindings · f file usages" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{} of {} bindings · f file usages" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Bindungen: {} / {} · f Dateireferenzen" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("{} / {} 个绑定 · f 文件引用" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Scan failed: {error}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Scan failed: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Scan fehlgeschlagen: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("扫描失败：{error}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Process folder scan failed: {error}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Process folder scan failed: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Prozessordner-Scan fehlgeschlagen: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("进程目录扫描失败：{error}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "Open browser: {error}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("Open browser: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("Browser öffnen: {error}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("打开浏览器：{error}" $(, $($arguments)*)?),
        }
    };
    ($language:expr, "{sent} termination requests sent{}" $(, $($arguments:tt)*)?) => {
        match $language {
            $crate::view::messages::Language::English => format!("{sent} termination requests sent{}" $(, $($arguments)*)?),
            $crate::view::messages::Language::German => format!("{sent} Beendigungsanfragen gesendet{}" $(, $($arguments)*)?),
            $crate::view::messages::Language::Chinese => format!("已发送 {sent} 个终止请求{}" $(, $($arguments)*)?),
        }
    };
}
pub(crate) use localized;

const ENGLISH_HELP: &str = "OPEN FILE LOCK HANDLE
1 / 2 / 3      Processes / locked files / ports
s              Ports: all ports / processes using this path
p / f          Details: ports / file usages
↑ / ↓, j       Navigate results
PgUp / PgDn    Move one page
Home / End     First / last result
Enter          Inspect matching paths
Space          Select process
Ctrl+A         Select / deselect all visible processes
/              Search PID, name, user, path, access
*              Wildcards: micro*dll, FLEC*.json
               Fragments and CamelCase; spaces combine terms.
Esc            Clear search / back / cancel
a              Toggle auto-refresh (5s after completion)
r / F5         Refresh (ignored during inspection)
z              Cancel an active inspection
i              Toggle side inspector
Tab / →        Focus ancestry; ↑↓ chooses action target
Tab / ← / Esc  Leave ancestry
l              Details: toggle locks only
m / c          Sort RAM / CPU descending
n / p          Sort name / PID
k / x          Stop / force kill selection or current process
K / X          Selection; otherwise all filtered processes
Tab            Choose Cancel / Terminate
?              Show help
R / D          Open repository / donation page
q / Ctrl+C     Quit (q types text while searching)

Every termination requires confirmation. Cancel is the default.
Hidden selections are included. Process identity is revalidated.
Stopping a parent does not recursively terminate its children.

PORT SEARCH
50 matches ports containing 50; port:3000 is exact; pid:123 matches a PID.
Combine terms: tcp 3000 server, udp, ipv6, or an address.
THIS PATH uses observed file references, not a guessed project name.
TCP LISTEN and UDP BOUND do not imply remote reachability.
Unknown owners have no actionable PID. Enter opens process details.

READING THE EVIDENCE
An open file is not necessarily locked.
locked         Platform lock / sharing-conflict evidence
open           Observed file descriptor
cwd            Current working directory
executable     Process executable
mapped         Mapped file / loaded module
restart manager  Windows resource user, owner unverified
native file user Windows file reference, owner unverified
unknown        OS did not expose this information

CPU is a share of total machine capacity, sampled twice.
Permissions, namespaces and races may limit visibility.";

const GERMAN_HELP: &str = "OPEN FILE LOCK HANDLE
1 / 2 / 3      Prozesse / gesperrte Dateien / Ports
s              Ports: alle / Prozesse mit Referenzen auf diesen Pfad
p / f          Details: Ports / Dateireferenzen
↑ / ↓, j       Ergebnisse durchgehen
PgUp / PgDn    Eine Seite bewegen
Home / End     Erstes / letztes Ergebnis
Enter          Passende Pfade untersuchen
Space          Prozess auswählen
Ctrl+A         Alle sichtbaren Prozesse auswählen / abwählen
/              PID, Name, Benutzer, Pfad, Zugriff suchen
*              Platzhalter: micro*dll, FLEC*.json
               Fragmente und CamelCase; Leerzeichen verknüpfen Begriffe.
Esc            Suche löschen / zurück / abbrechen
r / F5         Aktualisieren (während Untersuchung ignoriert)
a              Auto-Refresh umschalten (5s nach Abschluss)
z              Aktive Untersuchung abbrechen
i              Seitenleiste umschalten
Tab / →        Prozessbaum fokussieren; ↑↓ wählt Aktionsziel
Tab / ← / Esc  Prozessbaum verlassen
l              Details: nur Sperren umschalten
m / c          RAM / CPU absteigend sortieren
n / p          Nach Name / PID sortieren
k / x          Auswahl oder aktuellen Prozess beenden / erzwingen
K / X          Auswahl; sonst alle gefilterten Prozesse
Tab            Abbrechen / Beenden wählen
?              Hilfe anzeigen
R / D          Repository / Spendenseite öffnen
q / Ctrl+C     Beenden (q schreibt Text in der Suche)

Jedes Beenden erfordert Bestätigung. Abbrechen ist vorausgewählt.
Ausgefilterte Auswahl zählt mit. Die Prozessidentität wird erneut geprüft.
Ein Elternprozess wird ohne rekursives Beenden seiner Kinder gestoppt.

PORTSUCHE
50 findet Ports mit 50; port:3000 ist exakt; pid:123 sucht eine PID.
Begriffe kombinieren: tcp 3000 server, udp, ipv6 oder eine Adresse.
DIESER PFAD nutzt beobachtete Referenzen, keinen vermuteten Projektnamen.
TCP LISTEN und UDP BOUND beweisen keine Erreichbarkeit von außen.
Unbekannte Besitzer haben keine nutzbare PID. Enter öffnet Prozessdetails.

BELEGE RICHTIG LESEN
Eine offene Datei ist nicht zwangsläufig gesperrt.
gesperrt       Plattformsperre / Freigabekonflikt
Offen          Beobachteter Dateideskriptor
Arbeitsordner  Aktueller Arbeitsordner
Programm       Ausführbare Prozessdatei
zugeordnet     Zugeordnete Datei / geladenes Modul
Restart Manager  Windows-Ressourcenbenutzer; Besitzer unbestätigt
Dateinutzer (nativ)  Windows-Dateireferenz; Besitzer unbestätigt
unbekannt      Betriebssystem liefert diese Information nicht

CPU ist der Anteil an der gesamten Systemkapazität, zweimal gemessen.
Rechte, Namensräume und Änderungen können die Sichtbarkeit begrenzen.";

const CHINESE_HELP: &str = "OPEN FILE LOCK HANDLE
1 / 2 / 3      进程 / 锁定文件 / 端口
s              端口：所有端口 / 引用此路径的进程
p / f          详情：端口 / 文件引用
↑ / ↓, j       浏览结果
PgUp / PgDn    移动一页
Home / End     第一项 / 最后一项
Enter          检查匹配路径
Space          选择进程
Ctrl+A         选择 / 取消选择所有可见进程
/              搜索 PID、名称、用户、路径、访问方式
*              通配符：micro*dll、FLEC*.json
               支持片段和 CamelCase；空格连接搜索词。
Esc            清除搜索 / 返回 / 取消
r / F5         刷新（检查期间忽略）
a              切换自动刷新（完成后间隔 5 秒）
z              取消正在进行的检查
i              切换侧栏
Tab / →        聚焦进程树；↑↓ 选择操作目标
Tab / ← / Esc  离开进程树
l              详情：切换仅锁定项
m / c          按 RAM / CPU 降序排序
n / p          按名称 / PID 排序
k / x          终止 / 强制终止所选或当前进程
K / X          使用所选项；未选择时使用所有筛选结果
Tab            选择取消 / 终止
?              显示帮助
R / D          打开项目仓库 / 捐赠页面
q / Ctrl+C     退出（搜索中 q 用于输入文本）

每次终止都需要确认，默认选择取消。
被筛选条件隐藏的选择也包括在内。操作前会重新验证进程身份。
终止父进程不会递归终止其子进程。

端口搜索
50 匹配包含 50 的端口；port:3000 精确匹配；pid:123 匹配 PID。
组合搜索词：tcp 3000 server、udp、ipv6 或地址。
此路径使用已观察的文件引用，不猜测项目名称。
TCP LISTEN 和 UDP BOUND 不代表可从远程访问。
未知所有者没有可操作的 PID。Enter 打开进程详情。

理解证据
打开的文件不一定被锁定。
已锁定         平台文件锁 / 共享冲突证据
已打开         已观察的文件描述符
工作目录       当前工作目录
可执行文件     进程的可执行文件
映射           映射文件 / 已加载的模块
Restart Manager  Windows 资源使用者，所有者未经确认
本机文件引用者   Windows 文件引用，所有者未经确认
未知           操作系统未提供此信息

CPU 表示整机容量占比，通过两次采样计算。
权限、命名空间和并发变化可能限制可见性。";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_label_has_three_complete_unique_catalog_entries() {
        let mut keys = std::collections::HashSet::new();
        for &(english, german, chinese) in LABELS {
            assert!(keys.insert(english), "duplicate catalog entry");
            assert!(!english.trim().is_empty());
            assert!(!german.trim().is_empty());
            assert!(!chinese.trim().is_empty());
            assert_eq!(Language::English.text(english), english);
            assert_eq!(Language::German.text(english), german);
            assert_eq!(Language::Chinese.text(english), chinese);
            for label in [english, german, chinese] {
                assert!(!label.contains('\x1b'));
            }
        }
        assert_eq!(
            Language::German.text("native fixture name"),
            "native fixture name"
        );
    }
    #[test]
    fn locale_precedence_region_variants_and_unsupported_values_are_predictable() {
        for text in ["de", "de_DE.UTF-8", "DE-at", "de_DE@euro"] {
            assert_eq!(Language::resolve(text), Language::German);
        }
        for text in ["zh-CN", "zh_Hans", "zh-TW.UTF-8"] {
            assert_eq!(Language::resolve(text), Language::Chinese);
        }
        for text in ["C", "POSIX", "C.UTF-8", "fr_FR", "", "en-US"] {
            assert_eq!(Language::resolve(text), Language::English);
        }
        assert_eq!(
            Language::environment([Some("C"), Some("de"), Some("zh")]),
            Some(Language::English)
        );
        assert_eq!(
            Language::environment([Some(" "), Some("de"), Some("zh")]),
            Some(Language::German)
        );
        assert_eq!(
            Language::environment([None, None, Some("zh")]),
            Some(Language::Chinese)
        );
        assert_eq!(Language::environment([None, None, None]), None);
        assert!("invalid".parse::<Language>().is_err());
    }
    #[test]
    fn translated_formats_preserve_values_and_evaluate_arguments_once() {
        let mut calls = 0;
        let message = localized!(
            Language::German,
            "{} of {} processes",
            {
                calls += 1;
                2
            },
            10001
        );
        assert_eq!(calls, 1);
        assert_eq!(message, "Prozesse: 2 / 10001");
        assert_eq!(
            localized!(Language::Chinese, "{} of {} processes", 2, 10001),
            "2 / 10001 个进程"
        );
        let error = "native error 5";
        assert_eq!(
            localized!(Language::German, "Scan failed: {error}", error = error),
            "Scan fehlgeschlagen: native error 5"
        );
    }
}
