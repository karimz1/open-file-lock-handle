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
  --version     print version
  --help        show help";
const GERMAN_HELP: &str =
    "oflh — Open File Lock Handle. Finden Sie Prozesse, die Dateien und Ports verwenden.
Quellcode: https://github.com/karimz1/open-file-lock-handle

Aufruf: oflh [OPTIONS] [PATH]

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
  --version     Version ausgeben
  --help        Hilfe anzeigen";
const CHINESE_HELP: &str = "oflh — Open File Lock Handle. 查找正在使用文件和端口的进程。
源代码：https://github.com/karimz1/open-file-lock-handle

用法：oflh [OPTIONS] [PATH]

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
  --version     输出版本
  --help        显示帮助";
