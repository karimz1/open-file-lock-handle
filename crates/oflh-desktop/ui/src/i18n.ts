export type Locale = "en" | "de";

export function resolveLocale(language: string | null | undefined): Locale {
  const primaryLanguage = language?.trim().split(/[-_]/, 1)[0].toLowerCase();
  return primaryLanguage === "de" ? "de" : "en";
}

const german: Record<string, string> = {
  Workspace: "Arbeitsbereich",
  WORKSPACE: "ARBEITSBEREICH",
  "INSPECT TARGET": "ZIEL UNTERSUCHEN",
  Processes: "Prozesse",
  Process: "Prozess",
  processes: "Prozesse",
  process: "Prozess",
  "File usages": "Dateinutzungen",
  "File usage": "Dateinutzung",
  "Recent targets": "Zuletzt verwendete Ziele",
  Ports: "Ports",
  "Local ports": "Lokale Ports",
  "Open file": "Datei öffnen",
  "Open folder": "Ordner öffnen",
  Settings: "Einstellungen",
  Auto: "Automatisch",
  Off: "Aus",
  "Automatic refresh interval": "Intervall für automatische Aktualisierung",
  "Automatic refresh information":
    "Informationen zur automatischen Aktualisierung",
  "About automatic refresh": "Über die automatische Aktualisierung",
  "Automatic refresh": "Automatische Aktualisierung",
  "Off by default. Choose an interval to repeat the current scan while OFLH is open.":
    "Standardmäßig deaktiviert. Wähle ein Intervall, um die aktuelle Suche bei geöffnetem OFLH zu wiederholen.",
  "It can help with changing processes or ports. Results may change while you inspect, so manual refresh is often better for a focused check.":
    "Das kann bei veränderlichen Prozessen oder Ports hilfreich sein. Ergebnisse können sich während der Untersuchung ändern; für gezielte Prüfungen ist eine manuelle Aktualisierung oft besser.",
  Refresh: "Aktualisieren",
  or: "oder",
  "Star on GitHub": "Auf GitHub markieren",
  Development: "Entwicklung",
  "Operation could not complete":
    "Der Vorgang konnte nicht abgeschlossen werden",
  "Hide details": "Details ausblenden",
  Details: "Details",
  "Open issue": "Problem melden",
  "Review the draft and remove private paths before submitting.":
    "Prüfe den Entwurf und entferne private Pfade, bevor du ihn absendest.",
  "Dismiss error": "Fehler schließen",
  "NETWORK INSPECTION": "NETZWERKUNTERSUCHUNG",
  "FILE INSPECTION": "DATEIUNTERSUCHUNG",
  "Find local TCP listeners, bound UDP sockets and their captured owners.":
    "Finde lokale TCP-Listener, gebundene UDP-Sockets und die erfassten zugehörigen Prozesse.",
  "Processes referencing your target and its contents.":
    "Prozesse, die auf dein Ziel oder dessen Inhalte zugreifen.",
  "Find out which processes are using a file or folder.":
    "Finde heraus, welche Prozesse eine Datei oder einen Ordner verwenden.",
  "Target file or folder path": "Pfad zur Zieldatei oder zum Zielordner",
  "Paste a file or folder path…": "Datei- oder Ordnerpfad einfügen …",
  Inspect: "Untersuchen",
  "A clear view of files in use.":
    "Behalte den Überblick über verwendete Dateien.",
  "Drop a file or folder anywhere in this window.":
    "Ziehe eine Datei oder einen Ordner an eine beliebige Stelle in diesem Fenster.",
  "OFLH will find the processes referencing it.":
    "OFLH findet die Prozesse, die darauf zugreifen.",
  "Choose file": "Datei auswählen",
  "Choose folder": "Ordner auswählen",
  "Open files do not necessarily mean locked files.":
    "Geöffnete Dateien sind nicht unbedingt gesperrt.",
  "OFLH shows lock evidence when the operating system provides it.":
    "OFLH zeigt Hinweise auf Sperren an, sofern das Betriebssystem sie bereitstellt.",
  "Search loaded results": "Geladene Ergebnisse durchsuchen",
  "Search ports… e.g. 80, port:8080, tcp":
    "Ports durchsuchen … z. B. 80, port:8080, tcp",
  "Search names, PIDs, paths…": "Namen, PIDs und Pfade durchsuchen …",
  "Search ({shortcut}+F or /); Escape returns to the workspace":
    "Suchen ({shortcut}+F oder /); Escape kehrt zum Arbeitsbereich zurück",
  "Clear search": "Suche löschen",
  "Target processes only": "Nur Zielprozesse",
  "Lock evidence only": "Nur Sperrhinweise",
  "Select all": "Alle auswählen",
  Columns: "Spalten",
  "Visible columns": "Sichtbare Spalten",
  "SHOW IN GRID": "IN TABELLE ANZEIGEN",
  "Column filters": "Spaltenfilter",
  "Clear applied filters before closing":
    "Gesetzte Filter vor dem Schließen löschen",
  "Open column filters": "Spaltenfilter öffnen",
  results: "Ergebnisse",
  "Applied column filters": "Gesetzte Spaltenfilter",
  "Filters active": "Filter aktiv",
  "Clear filters": "Filter löschen",
  "Search includes full paths. A shared folder name can match every row; use the Process name column filter to narrow by name.":
    "Die Suche berücksichtigt vollständige Pfade. Ein gemeinsamer Ordnername kann auf alle Zeilen zutreffen. Grenze die Suche mit dem Spaltenfilter für Prozessnamen ein.",
  Showing: "Angezeigt werden",
  for: "für",
  "Clear process filter": "Prozessfilter löschen",
  selected: "ausgewählt",
  "May include processes outside this view":
    "Kann Prozesse außerhalb dieser Ansicht enthalten",
  Copy: "Kopieren",
  "Terminate…": "Beenden …",
  "Force terminate…": "Beenden erzwingen …",
  "Clear selection": "Auswahl aufheben",
  "Click a row to inspect · Use the panel button to close details":
    "Zeile anklicken, um Details anzuzeigen · Detailansicht über die Schaltfläche schließen",
  "Wait for the current scan to finish before opening row actions.":
    "Warte, bis die aktuelle Suche abgeschlossen ist, bevor du Zeilenaktionen öffnest.",
  "Local bindings do not prove external reachability. Unknown owners cannot be terminated.":
    "Lokale Bindungen belegen keine Erreichbarkeit von außen. Prozesse mit unbekanntem Besitzer können nicht beendet werden.",
  "File usage is not proof of a lock. Windows resource users are not proven lock owners.":
    "Dateinutzung ist kein Beweis für eine Sperre. Unter Windows sind Ressourcennutzer nicht als Sperrbesitzer bestätigt.",
  "coverage notices": "Hinweise zur Abdeckung",
  "SAVED HISTORY": "GESPEICHERTE VERLAUFSZIELE",
  "Reinspect targets saved on this device, even after reopening OFLH.":
    "Untersuche auf diesem Gerät gespeicherte Ziele erneut, auch nach einem Neustart von OFLH.",
  "Search recent targets": "Zuletzt verwendete Ziele durchsuchen",
  "Filter recent targets": "Zuletzt verwendete Ziele filtern",
  "Clear all": "Alle löschen",
  "Remove from recent targets": "Aus den zuletzt verwendeten Zielen entfernen",
  "No matching recent targets": "Keine passenden zuletzt verwendeten Ziele",
  "No recent targets": "Keine zuletzt verwendeten Ziele",
  "Choose a file or folder to start an inspection.":
    "Wähle eine Datei oder einen Ordner aus, um die Untersuchung zu starten.",
  PREFERENCES: "EINSTELLUNGEN",
  Appearance: "Darstellung",
  "Choose a theme or follow your system.":
    "Wähle ein Design oder übernimm die Systemeinstellung.",
  "Interface font size": "Schriftgröße der Oberfläche",
  "Scale text throughout the workspace, including tables and process details.":
    "Ändere die Textgröße im gesamten Arbeitsbereich, einschließlich Tabellen und Prozessdetails.",
  Size: "Größe",
  "Reset to default": "Auf Standard zurücksetzen",
  "About OFLH": "Über OFLH",
  "If OFLH helps your work, you can support its ongoing development.":
    "Wenn OFLH dir bei der Arbeit hilft, kannst du die Weiterentwicklung unterstützen.",
  "View project on GitHub": "Projekt auf GitHub ansehen",
  "View pull request": "Pull Request ansehen",
  "View Actions run": "Actions-Ausführung ansehen",
  "Keyboard shortcuts": "Tastenkürzel",
  "Processes / File usages / Ports / Recent targets":
    "Prozesse / Dateinutzungen / Ports / Zuletzt verwendete Ziele",
  "Next / previous workspace": "Nächster / vorheriger Arbeitsbereich",
  "Toggle selected process details":
    "Details des ausgewählten Prozesses umschalten",
  "Open file / folder": "Datei / Ordner öffnen",
  "Shift for folder": "Umschalttaste für Ordner",
  "Search results": "Ergebnisse durchsuchen",
  "Escape leaves search": "Escape beendet die Suche",
  "Refresh target": "Ziel aktualisieren",
  "Select all matching processes": "Alle passenden Prozesse auswählen",
  "Copy selected processes": "Ausgewählte Prozesse kopieren",
  "Navigate / toggle selection": "Navigieren / Auswahl umschalten",
  "Context actions": "Kontextaktionen",
  "Clear selection and details": "Auswahl und Details schließen",
  "Developer options": "Entwickleroptionen",
  "Preview the operation-error details and issue-report flow.":
    "Vorschau der Fehlerdetails und des Ablaufs zum Melden eines Problems.",
  "Show sample error": "Beispielfehler anzeigen",
  "Search and inspection": "Suche und Untersuchung",
  "Search runs over the loaded Rust snapshot and supports substrings, wildcards (*) and word-boundary abbreviations. In Ports, bare digits match port fragments; port:8080 matches exactly 8080. Combine port queries with tcp, udp, ipv4, ipv6 or pid:1234. Refresh performs a new system scan.":
    "Die Suche durchsucht die geladene Rust-Momentaufnahme und unterstützt Teilzeichenfolgen, Platzhalter (*) und Abkürzungen an Wortgrenzen. Unter Ports finden einzelne Ziffern auch Port-Teilzeichenfolgen; port:8080 findet genau 8080. Port-Suchen lassen sich mit tcp, udp, ipv4, ipv6 oder pid:1234 kombinieren. Aktualisieren startet eine neue Systemprüfung.",
  "Unknown CPU and memory stay unavailable. Access permissions may hide processes. Normal termination never escalates to force termination. Unsaved work can be lost when stopping a process.":
    "Unbekannte CPU- und Speicherwerte bleiben nicht verfügbar. Fehlende Berechtigungen können Prozesse verbergen. Ein normales Beenden wird niemals automatisch erzwungen. Beim Beenden eines Prozesses können ungespeicherte Daten verloren gehen.",
  Scanning: "Wird untersucht",
  "Inspection complete": "Untersuchung abgeschlossen",
  "Ready to inspect": "Bereit zur Untersuchung",
  Cancel: "Abbrechen",
  "file users": "Dateinutzer",
  "file usages": "Dateinutzungen",
  "Independent project by": "Unabhängiges Projekt von",
  "built in spare time": "in der Freizeit entwickelt",
  Donate: "Spenden",
  "Drop file or folder to inspect": "Datei oder Ordner zum Untersuchen ablegen",
  "One target at a time": "Jeweils ein Ziel",
  "Operation details": "Vorgangsdetails",
  "Diagnostic details can contain local paths or other private information. Review them before sharing.":
    "Diagnosedetails können lokale Pfade oder andere private Informationen enthalten. Prüfe sie, bevor du sie teilst.",
  "Copy details": "Details kopieren",
  Close: "Schließen",
  "Support OFLH": "OFLH unterstützen",
  "Support is optional. Choose the route that best fits how you would like to help this independent project.":
    "Unterstützung ist freiwillig. Wähle die Möglichkeit, die am besten zu deiner Unterstützung dieses unabhängigen Projekts passt.",
  "Buy Me a Coffee": "Buy Me a Coffee",
  "Good for:": "Geeignet für:",
  "a simple contribution from an individual.":
    "einen einfachen Beitrag von Einzelpersonen.",
  "Trade-off:": "Zu beachten:",
  "checkout is handled by a separate service.":
    "Die Zahlung wird über einen externen Dienst abgewickelt.",
  "Continue with Buy Me a Coffee": "Mit Buy Me a Coffee fortfahren",
  "GitHub Sponsors": "GitHub Sponsors",
  "ongoing sponsorship and company support.":
    "regelmäßige Unterstützung und Beiträge von Unternehmen.",
  "sponsorship uses GitHub’s checkout flow.":
    "Die Unterstützung wird über den Bezahlvorgang von GitHub abgewickelt.",
  "Continue to GitHub Sponsors": "Zu GitHub Sponsors wechseln",
  "Actions for PID": "Aktionen für PID",
  "Copy port": "Port kopieren",
  "Copy local endpoint": "Lokalen Endpunkt kopieren",
  "Copy path": "Pfad kopieren",
  "Copy filename": "Dateinamen kopieren",
  "Copy PID": "PID kopieren",
  "Copy process name": "Prozessnamen kopieren",
  "Reveal in file manager": "Im Dateimanager anzeigen",
  "Open containing folder": "Übergeordneten Ordner öffnen",
  "View process details": "Prozessdetails anzeigen",
  "View matching handles": "Passende Dateizugriffe anzeigen",
  "Make OFLH yours": "OFLH anpassen",
  "Choose your workspace theme. Preview it now; you can change it anytime in Settings.":
    "Wähle ein Design für deinen Arbeitsbereich. Du kannst es jederzeit in den Einstellungen ändern.",
  "Start inspecting": "Untersuchung starten",
  "Force terminate processes?": "Prozesse zwangsweise beenden?",
  "Terminate processes?": "Prozesse beenden?",
  "Force termination stops these processes without allowing normal cleanup. Unsaved work may be lost.":
    "Beim erzwungenen Beenden können diese Prozesse keine reguläre Bereinigung durchführen. Ungespeicherte Daten können verloren gehen.",
  "Request these processes to stop. Unsaved work may be lost. This will not escalate to force termination.":
    "Fordere diese Prozesse zum Beenden auf. Ungespeicherte Daten können verloren gehen. Das Beenden wird nicht erzwungen.",
  "all targets are listed below, including any hidden by filters.":
    "Alle Ziele sind unten aufgeführt, auch die durch Filter ausgeblendeten.",
  "OFLH validates each captured process identity again before sending the request.":
    "OFLH überprüft die Identität jedes erfassten Prozesses erneut, bevor die Anfrage gesendet wird.",
  "Waiting for process exit…": "Warte auf das Beenden der Prozesse …",
  Terminate: "Beenden",
  "Force terminate": "Beenden erzwingen",
  "Termination results": "Ergebnisse des Beendens",
  "Updating results…": "Ergebnisse werden aktualisiert …",
  "Results refreshed. Exit checks use the original process identity.":
    "Ergebnisse aktualisiert. Die Prüfung verwendet die ursprüngliche Prozessidentität.",
  "Process exited": "Prozess beendet",
  "Request sent · still running after 1.5 seconds":
    "Anfrage gesendet · nach 1,5 Sekunden noch aktiv",
  "Request sent · couldn’t verify exit":
    "Anfrage gesendet · Beenden konnte nicht überprüft werden",
  "Process changed or already exited. No termination was sent.":
    "Prozess geändert oder bereits beendet. Es wurde keine Beendigungsanfrage gesendet.",
  "Termination request failed": "Beendigungsanfrage fehlgeschlagen",
  "Refresh again": "Erneut aktualisieren",
  Done: "Fertig",
  "Process details": "Prozessdetails",
  "Close dialog": "Dialog schließen",
  "Resize process details": "Größe der Prozessdetails ändern",
  "Close process details": "Prozessdetails schließen",
  "PROCESS DETAILS": "PROZESSDETAILS",
  "Process inspection views": "Prozessansichten",
  "Matching handles": "Passende Dateizugriffe",
  "Process ancestry": "Prozesshierarchie",
  "Oldest captured parent → current process. Click a row to select its actions.":
    "Vom ältesten erfassten Elternprozess bis zum aktuellen Prozess. Wähle eine Zeile aus, um Aktionen anzuzeigen.",
  "Current process": "Aktueller Prozess",
  "(unavailable)": "(nicht verfügbar)",
  "No parent information available.":
    "Keine Informationen zum Elternprozess verfügbar.",
  Selected: "Ausgewählt",
  "Stopping a parent may close its children or your session.":
    "Das Beenden eines Elternprozesses kann dessen untergeordnete Prozesse oder deine Sitzung schließen.",
  "Terminate process…": "Prozess beenden …",
  "Terminate parent…": "Elternprozess beenden …",
  "Force terminate process…": "Prozess zwangsweise beenden …",
  "Force terminate parent…": "Elternprozess zwangsweise beenden …",
  "Protected process or unavailable identity.":
    "Geschützter Prozess oder Identität nicht verfügbar.",
  Memory: "Speicher",
  "CPU · total capacity": "CPU · Gesamtkapazität",
  Account: "Benutzerkonto",
  "Selected port": "Ausgewählter Port",
  "File observation": "Dateibeobachtung",
  "File was deleted": "Datei wurde gelöscht",
  Executable: "Programmdatei",
  "Working directory": "Arbeitsverzeichnis",
  "Inspect owner folder": "Ordner des Besitzers untersuchen",
  "Matching handles are target observations, not all handles of this process. Local ports include TCP listeners and UDP bindings.":
    "Passende Dateizugriffe beziehen sich auf das Ziel, nicht auf alle Zugriffe dieses Prozesses. Lokale Ports umfassen TCP-Listener und UDP-Bindungen.",
  "Resize {column} column": "Größe der Spalte {column} ändern",
  "Sort by": "Sortieren nach",
  "Local TCP listeners and UDP bindings":
    "Lokale TCP-Listener und UDP-Bindungen",
  "Matching file usages; selection applies to processes":
    "Passende Dateinutzungen; die Auswahl gilt für Prozesse",
  "Processes using this target": "Prozesse, die dieses Ziel verwenden",
  Loading: "Wird geladen",
  "Loading results": "Ergebnisse werden geladen",
  "No matching local ports": "Keine passenden lokalen Ports",
  "No matching processes": "Keine passenden Prozesse",
  "No rows match the current filters. Clear a filter to broaden the view.":
    "Keine Zeilen entsprechen den aktuellen Filtern. Lösche einen Filter, um mehr Ergebnisse anzuzeigen.",
  "No visible process references this target. Permission limits may hide some usage.":
    "Kein sichtbarer Prozess greift auf dieses Ziel zu. Fehlende Berechtigungen können Nutzungen verbergen.",
  "Open details": "Details öffnen",
  "Close details": "Details schließen",
  "details panel": "Detailansicht",
  deleted: "gelöscht",
  Unavailable: "Nicht verfügbar",
  "Process name": "Prozessname",
  "Exact PID": "Exakte PID",
  "Local address": "Lokale Adresse",
  "Full path": "Vollständiger Pfad",
  "Access / relation": "Zugriff / Beziehung",
  "Protocol / state": "Protokoll / Status",
  "CPU minimum (%)": "CPU-Minimum (%)",
  "CPU maximum (%)": "CPU-Maximum (%)",
  "Memory minimum (MiB)": "Speicherminimum (MiB)",
  "Memory maximum (MiB)": "Speichermaximum (MiB)",
  Evidence: "Nachweis",
  "Any · wildcards supported": "Beliebig · Platzhalter unterstützt",
  Any: "Beliebig",
  "Any evidence": "Beliebiger Nachweis",
  "Evidence present": "Nachweis vorhanden",
  "Kernel lock": "Kernel-Sperre",
  "Sharing conflict · owner unverified":
    "Freigabekonflikt · Besitzer unbestätigt",
  "No observed evidence": "Kein Nachweis beobachtet",
  "Use valid non-negative bounds; minimum must not exceed maximum.":
    "Gib gültige Werte ab null ein. Das Minimum darf das Maximum nicht überschreiten.",
  "Filters combine with search. Unknown metrics do not match numeric bounds.":
    "Filter werden mit der Suche kombiniert. Unbekannte Messwerte entsprechen keinen Zahlenbereichen.",
  "Close filters": "Filter schließen",
  "Clear applied filters to close": "Gesetzte Filter löschen, um zu schließen",
  "Apply filters": "Filter anwenden",
  System: "System",
  "Follow your device": "Systemeinstellung übernehmen",
  Light: "Hell",
  "Bright and clear": "Hell und klar",
  "Rider Dark": "Rider Dunkel",
  "Rider-inspired charcoal with crisp contrast":
    "Kohlefarben mit klarem Kontrast",
  "VS Code Dark": "VS Code Dunkel",
  "Graphite with blue accents": "Graphit mit blauen Akzenten",
  "OFLH Purple": "OFLH Violett",
  "Colors from the terminal demo": "Farben aus der Terminaldemo",
  Path: "Pfad",
  Access: "Zugriff",
  "CPU min": "CPU min.",
  "CPU max": "CPU max.",
  "Memory min": "Speicher min.",
  "Memory max": "Speicher max.",
  "Evidence / access": "Nachweis / Zugriff",
  "Filename copied": "Dateiname kopiert",
  "Selection copied": "Auswahl kopiert",
  "Path copied": "Pfad kopiert",
  "Error details copied": "Fehlerdetails kopiert",
  "No diagnostic details available.": "Keine Diagnosedetails verfügbar.",
  "Desktop operation": "Desktop-Vorgang",
  unknown: "unbekannt",
  read: "Lesen",
  write: "Schreiben",
  "read/write": "Lesen/Schreiben",
  execute: "Ausführen",
  directory: "Ordner",
  reference: "Referenz",
  mapped: "Speicherzuordnung",
  open: "Offen",
  cwd: "Arbeitsverzeichnis",
  executable: "Programmdatei",
  locked: "Gesperrt",
  "restart manager": "Restart Manager",
  LISTEN: "LAUSCHT",
  BOUND: "GEBUNDEN",
  "Some processes could not be inspected because access was denied.":
    "Einige Prozesse konnten wegen fehlender Zugriffsrechte nicht untersucht werden.",
  "Some port entries could not be inspected (permissions or socket changes).":
    "Einige Porteinträge konnten wegen fehlender Berechtigungen oder geänderter Sockets nicht untersucht werden.",
  "Some port owners are unavailable or changed during scanning; their ports are shown without an actionable PID.":
    "Einige Portbesitzer sind nicht verfügbar oder haben sich während der Suche geändert. Ihre Ports werden ohne verwendbare PID angezeigt.",
  "Ports are local TCP listeners and bound UDP sockets, not proof of remote reachability. Permissions and network namespaces limit visibility; container port forwarding is not enumerated.":
    "Ports zeigen lokale TCP-Listener und gebundene UDP-Sockets, belegen aber keine Erreichbarkeit von außen. Berechtigungen und Netzwerk-Namespaces schränken die Sichtbarkeit ein; Portweiterleitungen von Containern werden nicht erfasst.",
  "Know what’s using": "Finde heraus, was",
  "your files.": "deine Dateien verwendet.",
  "details for": "für",
  "Close details panel": "Detailansicht schließen",
  "Open details panel": "Detailansicht öffnen",
  Reveal: "Anzeigen",
  "Copy name": "Namen kopieren",
  "Selected file · full path": "Ausgewählte Datei · vollständiger Pfad",
  "local ports": "lokale Ports",
  of: "von",
  Remove: "Entfernen",
  "MIT license": "MIT-Lizenz",
  Open: "Öffnen",
  "GitHub profile": "GitHub-Profil",
  "Choose how to support OFLH": "Unterstützung für OFLH auswählen",
  "OFLH is an independent project by": "OFLH ist ein unabhängiges Projekt von",
  ", built in spare time. There is no company behind it.":
    ", das in der Freizeit entwickelt wird. Dahinter steht kein Unternehmen.",
  "## What happened?": "## Was ist passiert?",
  "OFLH reported:": "OFLH hat gemeldet:",
  "an operation could not complete":
    "Ein Vorgang konnte nicht abgeschlossen werden",
  "## Steps to reproduce": "## Schritte zur Reproduktion",
  "1. Open OFLH Desktop and inspect a file or folder.":
    "1. OFLH Desktop öffnen und eine Datei oder einen Ordner untersuchen.",
  "2. Right-click a process row (or open its details).":
    "2. Eine Prozesszeile rechts anklicken (oder die Details öffnen).",
  "3. Choose": "3. Auswählen:",
  "4. Note the result and any OS or file-manager dialog.":
    "4. Ergebnis und angezeigte Betriebssystem- oder Dateimanagerdialoge notieren.",
  "## Diagnostics": "## Diagnosedaten",
  "OFLH version:": "OFLH-Version:",
  "Platform:": "Plattform:",
  "No diagnostic details were provided.": "Keine Diagnosedetails angegeben.",
  "Please review this draft and remove any private paths or process details before submitting.":
    "Bitte prüfe den Entwurf und entferne private Pfade oder Prozessdetails vor dem Absenden.",
  "Desktop operation could not complete":
    "Desktop-Vorgang konnte nicht abgeschlossen werden",
};

export const locale = resolveLocale(
  typeof navigator === "undefined" ? undefined : navigator.language,
);

export function translate(language: Locale, message: string): string {
  return language === "de" ? (german[message] ?? message) : message;
}

export function t(message: string): string {
  return translate(locale, message);
}
