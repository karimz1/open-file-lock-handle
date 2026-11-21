//! Command-line presentation; option names, paths and build provenance stay stable.
use oflh_tui::Language;
pub fn choose(
    language: Language,
    english: &'static str,
    german: &'static str,
    chinese: &'static str,
) -> &'static str {
    match language {
        Language::English => english,
        Language::German => german,
        Language::Chinese => chinese,
    }
}
pub fn help(language: Language) -> &'static str {
    choose(language, ENGLISH_HELP, GERMAN_HELP, CHINESE_HELP)
}
const ENGLISH_HELP: &str = "oflh — Open File Lock Handle. See what's using your files and ports.
Source: https://github.com/karimz1/open-file-lock-handle

Usage: oflh [OPTIONS] [PATH]
       oflh check [OPTIONS] [PATH]
       oflh wait [--timeout SECONDS] [OPTIONS] [PATH]

  oflh check --json file  check once and print versioned JSON for scripts
  oflh check --port 3000  check whether local port 3000 has a binding
  oflh .                  inspect files in the current directory
  oflh --ports            inspect all visible local port bindings
  oflh --port 3000        find a TCP listener or UDP binding on port 3000
  oflh --ports .          inspect ports of processes using this directory
  oflh --language de .    use the German terminal interface

No PATH starts in the current directory; inspecting a port follows its owner folder.
An explicit PATH scopes Ports and keeps the file-inspection folder fixed.
Press s in Ports to switch between This path and All ports. Put options before PATH.

Options:
  --ports       start in the Ports tab (TCP listeners and bound UDP)
  --port PORT   start in Ports with an exact local-port filter
  --language en|de|zh|system   choose language; default follows the system locale
  wait PATH       poll until free; 0 free, 4 timeout, 130 cancelled
  --wait-timeout SECONDS   optional deadline for the interactive w command
  --timeout SECONDS / --poll-interval SECONDS   configure wait
  --no-update-check   disable network release checks
  --version     print version
  --help        show help

Check exit codes: 0 free, 1 held, 2 usage error, 3 inspection failed.
JSON relation/access/lock values use the same stable labels as the terminal.
Paths that are not valid UTF-8 use the Unicode replacement character.";
const GERMAN_HELP: &str =
    "oflh — Open File Lock Handle. Finden Sie Prozesse, die Dateien und Ports verwenden.
Quellcode: https://github.com/karimz1/open-file-lock-handle

Aufruf: oflh [OPTIONS] [PATH]
        oflh check [OPTIONS] [PATH]
       oflh wait [--timeout SECONDS] [OPTIONS] [PATH]

  oflh check --json Datei  einmal prüfen und versioniertes JSON für Skripte ausgeben
  oflh check --port 3000   prüfen, ob lokaler Port 3000 gebunden ist
  oflh .                  Dateien im aktuellen Ordner untersuchen
  oflh --ports            alle sichtbaren lokalen Portbindungen untersuchen
  oflh --port 3000        TCP-Listener oder UDP-Bindung auf Port 3000 finden
  oflh --ports .          Ports der Prozesse mit Referenzen auf diesen Ordner
  oflh --language de .    deutsche Terminaloberfläche verwenden

Ohne PATH startet oflh im aktuellen Ordner; ein Port folgt dem Ordner seines Besitzers.
Ein expliziter PATH begrenzt Ports und hält den Dateiordner fest.
Mit s zwischen diesem Pfad und allen Ports wechseln. Optionen vor PATH angeben.

Optionen:
  --ports       in der Portansicht starten (TCP-Listener und gebundenes UDP)
  --port PORT   Portansicht mit exaktem lokalen Portfilter öffnen
  --language en|de|zh|system   Sprache wählen; Standard folgt der System-Locale
  wait            auf Freigabe warten; 0 Erfolg, 4 Zeitgrenze, 130 abgebrochen
  --wait-timeout SEKUNDEN   optionale Grenze für den interaktiven w-Befehl
  --timeout SEKUNDEN / --poll-interval SEKUNDEN   Warten steuern
  --no-update-check   Netzwerkprüfung auf neue Versionen deaktivieren
  --version     Version ausgeben
  --help        Hilfe anzeigen

Exit-Codes von check: 0 frei, 1 belegt, 2 falscher Aufruf, 3 Prüfung fehlgeschlagen.
JSON-Werte für Relation/Zugriff/Sperre entsprechen den Terminalbezeichnungen.
Nicht UTF-8-konforme Pfade verwenden das Unicode-Ersatzzeichen.";
const CHINESE_HELP: &str = "oflh — Open File Lock Handle. 查找正在使用文件和端口的进程。
源代码：https://github.com/karimz1/open-file-lock-handle

用法：oflh [OPTIONS] [PATH]
       oflh check [OPTIONS] [PATH]
       oflh wait [--timeout SECONDS] [OPTIONS] [PATH]

  oflh check --json 文件  检查一次并为脚本输出带版本的 JSON
  oflh check --port 3000  检查本地端口 3000 是否有绑定
  oflh .                  检查当前目录中的文件
  oflh --ports            检查所有可见的本地端口绑定
  oflh --port 3000        查找端口 3000 上的 TCP 侦听器或 UDP 绑定
  oflh --ports .          检查引用此目录的进程所使用的端口
  oflh --language zh .    使用简体中文终端界面

未提供 PATH 时从当前目录开始；检查端口会跟随所有者的目录。
显式 PATH 会限定端口范围并保持文件检查目录不变。
在端口视图中按 s 切换此路径与所有端口。请在 PATH 前指定选项。

选项：
  --ports       从端口视图启动（TCP 侦听器和已绑定的 UDP）
  --port PORT   从端口视图启动，并精确筛选本地端口
  --language en|de|zh|system   选择语言；默认跟随系统区域设置
  wait            等待目标空闲；0 成功，4 超时，130 已中断
  --wait-timeout 秒   交互式 w 命令的可选等待时限
  --timeout 秒 / --poll-interval 秒   设置等待选项
  --no-update-check   禁用联网版本检查
  --version     输出版本
  --help        显示帮助

check 退出码：0 空闲，1 已占用，2 参数错误，3 检查失败。
JSON 的 relation/access/lock 值与终端使用相同的稳定标签。
非 UTF-8 路径使用 Unicode 替换字符。";
