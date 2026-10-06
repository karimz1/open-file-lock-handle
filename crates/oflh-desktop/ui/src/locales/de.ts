import type { TranslationSchema } from "../i18n/types";
import en from "./en";

const de: TranslationSchema<typeof en> = {
  about: {
    k_version: "Version",
    k_commit: "Commit",
    k_system: "System",
    k_license: "Lizenz",
    k_copied: "Informationen kopiert.",
  },
  app: {
    k_installed_version: "Installierte Version {{version}}",
    k_view_rc_pipeline: "Pipeline für RC ansehen",
    k_2_right_click_a_process_row_or_open_its_details:
      "2. Eine Prozesszeile rechts anklicken (oder die Details öffnen).",
    k_3_choose: "3. Auswählen:",
    k_actions_for_pid: "Aktionen für PID",
    k_auto: "Automatisch",
    k_bound: "GEBUNDEN",
    k_built_in_spare_time: "in der Freizeit entwickelt",
    k_built_in_spare_time_there_is_no_company_5cd7cc08:
      ", das in der Freizeit entwickelt wird. Dahinter steht kein Unternehmen.",
    k_clear_all: "Alle löschen",
    k_close_details_panel: "Detailansicht schließen",
    k_close_dialog: "Dialog schließen",
    k_context_actions: "Kontextaktionen",
    k_cpu: "CPU",
    k_cpu_max: "CPU max.",
    k_cpu_min: "CPU min.",
    k_current_process: "Aktueller Prozess",
    k_cwd: "Arbeitsverzeichnis",
    k_desktop: "Desktop",
    k_desktop_operation: "Desktop-Vorgang",
    k_details_for: "für",
    k_details_panel: "Detailansicht",
    k_developer_options: "Entwickleroptionen",
    k_development: "Entwicklung",
    k_directory: "Ordner",
    k_dismiss_error: "Fehler schließen",
    k_exact_pid: "Exakte PID",
    k_execute: "Ausführen",
    k_file_usage: "Dateinutzung",
    k_file_usage_is_not_proof_of_a_lock_windo_f7640915:
      "Dateinutzung ist kein Beweis für eine Sperre. Unter Windows sind Ressourcennutzer nicht als Sperrbesitzer bestätigt.",
    k_for: "für",
    k_full_path: "Vollständiger Pfad",
    k_github_profile: "GitHub-Profil",
    k_independent_project_by: "Unabhängiges Projekt von",
    k_kernel_lock: "Kernel-Sperre",
    k_know_what_s_using: "Finde heraus, was",
    k_listen: "LAUSCHT",
    k_loading: "Wird geladen",
    k_local_address: "Lokale Adresse",
    k_locked: "Gesperrt",
    k_make_oflh_yours: "OFLH anpassen",
    k_mapped: "Speicherzuordnung",
    k_mit_license: "MIT-Lizenz",
    k_navigate_toggle_selection: "Navigieren / Auswahl umschalten",
    k_of: "von",
    k_oflh_is_an_independent_project_by:
      "OFLH ist ein unabhängiges Projekt von",
    k_oflh_reported: "OFLH hat gemeldet:",
    k_open: "Offen",
    k_open_cf9b7706: "Öffnen",
    k_open_containing_folder: "Übergeordneten Ordner öffnen",
    k_open_details_panel: "Detailansicht öffnen",
    k_path: "Pfad",
    k_preferences: "EINSTELLUNGEN",
    k_process: "Prozess",
    k_process_c2e2d662: "Prozess",
    k_process_name: "Prozessname",
    k_read: "Lesen",
    k_read_write: "Lesen/Schreiben",
    k_reference: "Referenz",
    k_remove: "Entfernen",
    k_restart_manager: "Restart Manager",
    k_native_file_user: "Nutzer laut Windows-Dateiabfrage",
    k_results: "Ergebnisse",
    k_reveal_in_file_manager: "Im Dateimanager anzeigen",
    k_pid: "PID",
    k_port: "Port",
    k_sharing_conflict_delete_denied_reported_9ec719fc:
      "FREIGABEKONFLIKT: Löschen verweigert; gemeldeter Dateinutzer, Sperrbesitzer nicht bestätigt",
    k_sharing_conflict_owner_unverified:
      "Freigabekonflikt · Besitzer unbestätigt",
    k_sharing_conflict_read_denied_reported_f_46f22ec1:
      "FREIGABEKONFLIKT: Lesen verweigert; gemeldeter Dateinutzer, Sperrbesitzer nicht bestätigt",
    k_sharing_conflict_write_denied_reported_5b8637bb:
      "FREIGABEKONFLIKT: Schreiben verweigert; gemeldeter Dateinutzer, Sperrbesitzer nicht bestätigt",
    k_shift_for_folder: "Umschalttaste für Ordner",
    k_showing: "Angezeigt werden",
    k_size: "Größe",
    k_some_port_entries_could_not_be_inspecte_6356812f:
      "Einige Porteinträge konnten wegen fehlender Berechtigungen oder geänderter Sockets nicht untersucht werden.",
    k_apply_theme: "Theme anwenden",
    k_unknown: "unbekannt",
    k_unnamed: "(unbenannt)",
    k_view_actions_run: "Actions-Ausführung ansehen",
    k_view_project_on_github: "Projekt auf GitHub ansehen",
    k_view_pull_request: "Pull Request ansehen",
    k_waiting_for_process_exit: "Warte auf das Beenden der Prozesse …",
    k_what_happened: "## Was ist passiert?",
    k_write: "Schreiben",
    k_your_files: "deine Dateien verwendet.",
  },
  common: {
    k_any: "Beliebig",
    k_cancel: "Abbrechen",
    k_close: "Schließen",
    k_close_details: "Details schließen",
    k_details: "Details",
    k_done: "Fertig",
    k_hide_details: "Details ausblenden",
    k_off: "Aus",
    k_open_details: "Details öffnen",
    k_operation_details: "Vorgangsdetails",
    k_or: "oder",
    k_reveal: "Anzeigen",
    k_unavailable: "(nicht verfügbar)",
    k_unavailable_2c9c1f79: "Nicht verfügbar",
  },
  diagnostics: {
    k_4_note_the_result_and_any_os_or_file_ma_9ec676ec:
      "4. Ergebnis und angezeigte Betriebssystem- oder Dateimanagerdialoge notieren.",
    k_an_operation_could_not_complete:
      "Ein Vorgang konnte nicht abgeschlossen werden",
    k_desktop_operation_could_not_complete:
      "Desktop-Vorgang konnte nicht abgeschlossen werden",
    k_diagnostic_details_can_contain_local_pa_5bc47018:
      "Diagnosedetails können lokale Pfade oder andere private Informationen enthalten. Prüfe sie, bevor du sie teilst.",
    k_diagnostics: "## Diagnosedaten",
    k_no_diagnostic_details_available: "Keine Diagnosedetails verfügbar.",
    k_no_diagnostic_details_were_provided: "Keine Diagnosedetails angegeben.",
    k_oflh_version: "OFLH-Version:",
    k_open_issue: "Problem melden",
    k_operation_could_not_complete:
      "Der Vorgang konnte nicht abgeschlossen werden",
    k_platform: "Plattform:",
    k_please_review_this_draft_and_remove_any_3d3c9702:
      "Bitte prüfe den Entwurf und entferne private Pfade oder Prozessdetails vor dem Absenden.",
    k_preview_the_operation_error_details_and_667dc1ef:
      "Vorschau der Fehlerdetails und des Ablaufs zum Melden eines Problems.",
    k_review_the_draft_and_remove_private_pat_831914fd:
      "Prüfe den Entwurf und entferne private Pfade, bevor du ihn absendest.",
    k_show_sample_error: "Beispielfehler anzeigen",
    k_steps_to_reproduce: "## Schritte zur Reproduktion",
  },
  filters: {
    k_access_relation: "Zugriff / Beziehung",
    k_all_targets_are_listed_below_including_1515b80a:
      "Alle Ziele sind unten aufgeführt, auch die durch Filter ausgeblendeten.",
    k_any_evidence: "Beliebiger Nachweis",
    k_applied_column_filters: "Gesetzte Spaltenfilter",
    k_apply_filters: "Filter anwenden",
    k_clear_applied_filters_before_closing:
      "Gesetzte Filter vor dem Schließen löschen",
    k_clear_applied_filters_to_close:
      "Gesetzte Filter löschen, um zu schließen",
    k_clear_filters: "Filter löschen",
    k_clear_process_filter: "Prozessfilter löschen",
    k_close_filters: "Filter schließen",
    k_column_filters: "Spaltenfilter",
    k_cpu_maximum: "CPU-Maximum (%)",
    k_cpu_minimum: "CPU-Minimum (%)",
    k_evidence: "Nachweis",
    k_evidence_access: "Nachweis / Zugriff",
    k_evidence_present: "Nachweis vorhanden",
    k_filters_active: "Filter aktiv",
    k_filters_combine_with_search_unknown_met_4d1f7e8f:
      "Filter werden mit der Suche kombiniert. Unbekannte Messwerte entsprechen keinen Zahlenbereichen.",
    k_lock_evidence_only: "Nur Sperrhinweise",
    k_memory_maximum_mib: "Speichermaximum (MiB)",
    k_memory_minimum_mib: "Speicherminimum (MiB)",
    k_no_observed_evidence: "Kein Nachweis beobachtet",
    k_no_rows_match_the_current_filters_clear_0d9d0c2e:
      "Keine Zeilen entsprechen den aktuellen Filtern. Lösche einen Filter, um mehr Ergebnisse anzuzeigen.",
    k_oflh_shows_lock_evidence_when_the_opera_adfdf7ef:
      "OFLH zeigt Hinweise auf Sperren an, sofern das Betriebssystem sie bereitstellt.",
    k_open_column_filters: "Spaltenfilter öffnen",
    k_search_includes_full_paths_a_shared_fol_1d2d885e:
      "Die Suche berücksichtigt vollständige Pfade. Ein gemeinsamer Ordnername kann auf alle Zeilen zutreffen. Grenze die Suche mit dem Spaltenfilter für Prozessnamen ein.",
    k_use_valid_non_negative_bounds_minimum_m_0eb65520:
      "Gib gültige Werte ab null ein. Das Minimum darf das Maximum nicht überschreiten.",
  },
  history: {
    k_filter_recent_targets: "Zuletzt verwendete Ziele filtern",
    k_no_matching_recent_targets: "Keine passenden zuletzt verwendeten Ziele",
    k_no_recent_targets: "Keine zuletzt verwendeten Ziele",
    k_processes_file_usages_ports_recent_targets:
      "Prozesse / Dateinutzungen / Ports / Zuletzt verwendete Ziele",
    k_recent_targets: "Zuletzt verwendete Ziele",
    k_reinspect_targets_saved_on_this_device_b58bafd9:
      "Untersuche auf diesem Gerät gespeicherte Ziele erneut, auch nach einem Neustart von OFLH.",
    k_remove_from_recent_targets:
      "Aus den zuletzt verwendeten Zielen entfernen",
    k_saved_history: "GESPEICHERTE VERLAUFSZIELE",
    k_search_recent_targets: "Zuletzt verwendete Ziele durchsuchen",
  },
  inspection: {
    k_checking_process_references: "Prozessreferenzen werden geprüft",
    k_checking_files: "Dateien werden geprüft",
    k_checking_ports: "Lokale Ports werden geprüft",
    k_preparing_results: "Ergebnisse werden vorbereitet",
    k_reload_paused: "Neuladen ist bis zum Ende der Untersuchung pausiert.",
    k_starting_inspection: "Untersuchung wird gestartet…",
    k_files_checked: "{{count}} Dateien geprüft",
    k_references_checked: "{{count}} Referenzen geprüft",
    k_processes_checked: "{{count}} Prozesse geprüft",
    k_unknown_total: "Die Gesamtzahl ist während der Untersuchung unbekannt.",
    k_progress_unavailable:
      "Fortschritt ist vorübergehend nicht verfügbar. Die Untersuchung läuft weiter.",

    k_1_open_oflh_desktop_and_inspect_a_file_or_folder:
      "1. OFLH Desktop öffnen und eine Datei oder einen Ordner untersuchen.",
    k_a_clear_view_of_files_in_use:
      "Behalte den Überblick über verwendete Dateien.",
    k_choose_a_file_or_folder_to_start_an_inspection:
      "Wähle eine Datei oder einen Ordner aus, um die Untersuchung zu starten.",
    k_choose_file: "Datei auswählen",
    k_choose_folder: "Ordner auswählen",
    k_drop_a_file_or_folder_anywhere_in_this_window:
      "Ziehe eine Datei oder einen Ordner an eine beliebige Stelle in diesem Fenster.",
    k_drop_file_or_folder_to_inspect:
      "Datei oder Ordner zum Untersuchen ablegen",
    k_file_inspection: "DATEIUNTERSUCHUNG",
    k_find_out_which_processes_are_using_a_fi_c1cc0eee:
      "Finde heraus, welche Prozesse eine Datei oder einen Ordner verwenden.",
    k_inspect: "Untersuchen",
    k_inspect_target: "ZIEL UNTERSUCHEN",
    k_inspection_complete: "Untersuchung abgeschlossen",
    k_local_ports: "Lokale Ports",
    k_local_ports_903c8be8: "lokale Ports",
    k_network_inspection: "NETZWERKUNTERSUCHUNG",
    k_off_by_default_choose_an_interval_to_re_fb2f1749:
      "Standardmäßig deaktiviert. Wähle ein Intervall, um die aktuelle Suche bei geöffnetem OFLH zu wiederholen.",
    k_oflh_will_find_the_processes_referencing_it:
      "OFLH findet die Prozesse, die darauf zugreifen.",
    k_one_target_at_a_time: "Jeweils ein Ziel",
    k_open_file: "Datei öffnen",
    k_open_file_folder: "Datei / Ordner öffnen",
    k_open_files_do_not_necessarily_mean_locked_files:
      "Geöffnete Dateien sind nicht unbedingt gesperrt.",
    k_open_folder: "Ordner öffnen",
    k_paste_a_file_or_folder_path: "Datei- oder Ordnerpfad einfügen …",
    k_process_inspection_views: "Prozessansichten",
    k_processes_referencing_your_target_and_i_9e70e943:
      "Prozesse, die auf dein Ziel oder dessen Inhalte zugreifen.",
    k_ready_to_inspect: "Bereit zur Untersuchung",
    k_refresh_target: "Ziel aktualisieren",
    k_scanning: "Wird untersucht",
    k_some_port_owners_are_unavailable_or_cha_51384a8d:
      "Einige Portbesitzer sind nicht verfügbar oder haben sich während der Suche geändert. Ihre Ports werden ohne verwendbare PID angezeigt.",
    k_target_file_or_folder_path: "Pfad zur Zieldatei oder zum Zielordner",
    k_target_processes_only: "Nur Zielprozesse",
    k_wait_for_the_current_scan_to_finish_bef_20cd41dd:
      "Warte, bis die aktuelle Suche abgeschlossen ist, bevor du Zeilenaktionen öffnest.",
    k_updating_results: "Ergebnisse werden aktualisiert…",
    k_results_refreshed: "Ergebnisse aktualisiert",
    k_background_hint:
      "Die bisherigen Ergebnisse bleiben während der Aktualisierung verfügbar. Ein weiterer Scan wartet auf den Abschluss.",
    k_cancel_refresh: "Automatische Aktualisierung abbrechen",
    k_updating_details:
      "Diese Details werden aktualisiert. Aktionen sind mit dem neuen Ergebnis wieder verfügbar.",
    k_captured_process_missing:
      "Dieser erfasste Prozess fehlt in der aktuellen Untersuchung. Er wurde möglicherweise beendet oder verwendet dieses Ziel nicht mehr.",
    k_captured_observation:
      "Diese Beobachtung stammt aus der vorherigen Untersuchung. Wähle eine aktuelle Zeile für Dateiaktionen.",
  },
  inspector: {
    k_access: "Zugriff",
    k_account: "Benutzerkonto",
    k_close_process_details: "Prozessdetails schließen",
    k_cpu_total_capacity: "CPU · Gesamtkapazität",
    k_executable: "Programmdatei",
    k_executable_0d5602f9: "Programmdatei",
    k_file_observation: "Dateibeobachtung",
    k_find_local_tcp_listeners_bound_udp_sock_7d6edd16:
      "Finde lokale TCP-Listener, gebundene UDP-Sockets und die erfassten zugehörigen Prozesse.",
    k_inspect_owner_folder: "Ordner des Besitzers untersuchen",
    k_local_tcp_listeners_and_udp_bindings:
      "Lokale TCP-Listener und UDP-Bindungen",
    k_matching_file_usages_selection_applies_56df1d76:
      "Passende Dateinutzungen; die Auswahl gilt für Prozesse",
    k_matching_handles: "Passende Dateizugriffe",
    k_matching_handles_are_target_observation_13c2ef67:
      "Passende Dateizugriffe beziehen sich auf das Ziel, nicht auf alle Zugriffe dieses Prozesses. Lokale Ports umfassen TCP-Listener und UDP-Bindungen.",
    k_memory: "Speicher",
    k_memory_max: "Speicher max.",
    k_memory_min: "Speicher min.",
    k_no_parent_information_available:
      "Keine Informationen zum Elternprozess verfügbar.",
    k_oldest_captured_parent_current_process_79d8937a:
      "Vom ältesten erfassten Elternprozess bis zum aktuellen Prozess. Wähle eine Zeile aus, um Aktionen anzuzeigen.",
    k_ports_are_local_tcp_listeners_and_bound_2b2ed776:
      "Ports zeigen lokale TCP-Listener und gebundene UDP-Sockets, belegen aber keine Erreichbarkeit von außen. Berechtigungen und Netzwerk-Namespaces schränken die Sichtbarkeit ein; Portweiterleitungen von Containern werden nicht erfasst.",
    k_process_ancestry: "Prozesshierarchie",
    k_process_details: "Prozessdetails",
    k_process_details_2d6f7b50: "PROZESSDETAILS",
    k_processes_using_this_target: "Prozesse, die dieses Ziel verwenden",
    k_resize_process_details: "Größe der Prozessdetails ändern",
    k_scale_text_throughout_the_workspace_inc_945ceeb1:
      "Ändere die Textgröße im gesamten Arbeitsbereich, einschließlich Tabellen und Prozessdetails.",
    k_selected_file_full_path: "Ausgewählte Datei · vollständiger Pfad",
    k_selected_port: "Ausgewählter Port",
    k_some_processes_could_not_be_inspected_b_a1aee461:
      "Einige Prozesse konnten wegen fehlender Zugriffsrechte nicht untersucht werden.",
    k_toggle_selected_process_details:
      "Details des ausgewählten Prozesses umschalten",
    k_view_matching_handles: "Passende Dateizugriffe anzeigen",
    k_view_process_details: "Prozessdetails anzeigen",
    k_working_directory: "Arbeitsverzeichnis",
  },
  language: {
    k_chinese_simplified: "Chinesisch (vereinfacht)",
    k_choose_a_language_or_follow_your_system_setting:
      "Wähle eine Sprache oder übernimm die Systemeinstellung.",
    k_english: "Englisch",
    k_german: "Deutsch",
    k_language: "Sprache",
    k_system_default: "Systemstandard",
  },
  navigation: {
    k_collapse_sidebar: "Seitenleiste einklappen",
    k_expand_sidebar: "Seitenleiste ausklappen",
    k_next_previous_workspace: "Nächster / vorheriger Arbeitsbereich",
    k_ports: "Ports",
    k_processes: "Prozesse",
    k_processes_da2c4eba: "Prozesse",
    k_settings: "Einstellungen",
    k_star_on_github: "Stern auf GitHub vergeben",
    k_workspace: "Arbeitsbereich",
    k_workspace_70398828: "ARBEITSBEREICH",
  },
  search: {
    k_clear_search: "Suche löschen",
    k_escape_leaves_search: "Escape beendet die Suche",
    k_search_and_inspection: "Suche und Untersuchung",
    k_search_loaded_results: "Geladene Ergebnisse durchsuchen",
    k_search_names_pids_paths: "Namen, PIDs und Pfade durchsuchen …",
    k_search_ports_e_g_80_port_8080_tcp:
      "Ports durchsuchen … z. B. 80, port:8080, tcp",
    k_search_results: "Ergebnisse durchsuchen",
    k_search_shortcut_f_or_escape_returns_to_e2053cec:
      "Suchen ({{shortcut}}+F oder /); Escape kehrt zum Arbeitsbereich zurück",
  },
  selection: {
    k_clear_selection: "Auswahl aufheben",
    k_clear_selection_and_details: "Auswahl und Details schließen",
    k_click_a_row_to_inspect_use_the_panel_bu_1ffc65e8:
      "Zeile anklicken, um Details anzuzeigen · Detailansicht über die Schaltfläche schließen",
    k_copy: "Kopieren",
    k_copy_details: "Details kopieren",
    k_copy_filename: "Dateinamen kopieren",
    k_copy_local_endpoint: "Lokalen Endpunkt kopieren",
    k_copy_name: "Namen kopieren",
    k_copy_path: "Pfad kopieren",
    k_copy_pid: "PID kopieren",
    k_copy_port: "Port kopieren",
    k_copy_process_name: "Prozessnamen kopieren",
    k_copy_selected_processes: "Ausgewählte Prozesse kopieren",
    k_error_details_copied: "Fehlerdetails kopiert",
    k_filename_copied: "Dateiname kopiert",
    k_may_include_processes_outside_this_view:
      "Kann Prozesse außerhalb dieser Ansicht enthalten",
    k_path_copied: "Pfad kopiert",
    k_select_all: "Alle auswählen",
    k_select_all_matching_processes: "Alle passenden Prozesse auswählen",
    k_selected: "ausgewählt",
    k_selected_9a976fc2: "Ausgewählt",
    k_selection_copied: "Auswahl kopiert",
  },
  settings: {
    k_about_oflh: "Über OFLH",
    k_appearance: "Darstellung",
    k_interface_font_size: "Schriftgröße der Oberfläche",
    k_interface_zoom: "Zoomstufe der Oberfläche",
    k_keyboard_shortcuts: "Tastenkürzel",
    k_reset_to_default: "Auf Standard zurücksetzen",
    k_scale_the_whole_interface_zoom_help:
      "Skaliert die gesamte Oberfläche, einschließlich Symbole, Schaltflächen und Abstände. Hilfreich, wenn kleine Bedienelemente schwer lesbar sind.",
    k_zoom_in: "Vergrößern",
    k_zoom_in_out_reset: "Vergrößern / Verkleinern / Zurücksetzen",
    k_zoom_out: "Verkleinern",
  },
  status: {
    k_about_automatic_refresh: "Über die automatische Aktualisierung",
    k_automatic_refresh: "Automatische Aktualisierung",
    k_automatic_refresh_information:
      "Informationen zur automatischen Aktualisierung",
    k_automatic_refresh_interval: "Intervall für automatische Aktualisierung",
    k_coverage_notices: "Hinweise zur Abdeckung",
    k_file_usages: "Dateinutzungen",
    k_file_usages_d01933d6: "Dateinutzungen",
    k_file_users: "Dateinutzer",
    k_it_can_help_with_changing_processes_or_f5538b6f:
      "Das kann bei veränderlichen Prozessen oder Ports hilfreich sein. Ergebnisse können sich während der Untersuchung ändern; für gezielte Prüfungen ist eine manuelle Aktualisierung oft besser.",
    k_refresh: "Aktualisieren",
    k_refresh_again: "Erneut aktualisieren",
    k_results_refreshed_exit_checks_use_the_o_d1d684f4:
      "Ergebnisse aktualisiert. Die Prüfung verwendet die ursprüngliche Prozessidentität.",
    k_updating_results: "Ergebnisse werden aktualisiert …",
  },
  support: {
    k_paypal_contribution: "Einen Beitrag über PayPal senden.",
    k_continue_to_paypal: "Weiter zu PayPal",
    k_a_simple_contribution_from_an_individual:
      "einen einfachen Beitrag von Einzelpersonen.",
    k_any_wildcards_supported: "Beliebig · Platzhalter unterstützt",
    k_buy_me_a_coffee: "Buy Me a Coffee",
    k_checkout_is_handled_by_a_separate_service:
      "Die Zahlung wird über einen externen Dienst abgewickelt.",
    k_choose_how_to_support_oflh: "Unterstützung für OFLH auswählen",
    k_continue_to_github_sponsors: "Zu GitHub Sponsors wechseln",
    k_continue_with_buy_me_a_coffee: "Mit Buy Me a Coffee fortfahren",
    k_donate: "Spenden",
    k_github_sponsors: "GitHub Sponsors",
    k_good_for: "Geeignet für:",
    k_if_oflh_helps_your_work_you_can_support_4c5c07a8:
      "Wenn OFLH dir bei der Arbeit hilft, kannst du die Weiterentwicklung unterstützen.",
    k_ongoing_sponsorship_and_company_support:
      "regelmäßige Unterstützung und Beiträge von Unternehmen.",
    k_search_runs_over_the_loaded_rust_snapsh_b52f0264:
      "Die Suche durchsucht die geladene Rust-Momentaufnahme und unterstützt Teilzeichenfolgen, Platzhalter (*) und Abkürzungen an Wortgrenzen. Unter Ports finden einzelne Ziffern auch Port-Teilzeichenfolgen; port:8080 findet genau 8080. Port-Suchen lassen sich mit tcp, udp, ipv4, ipv6 oder pid:1234 kombinieren. Aktualisieren startet eine neue Systemprüfung.",
    k_sponsorship_uses_github_s_checkout_flow:
      "Die Unterstützung wird über den Bezahlvorgang von GitHub abgewickelt.",
    k_support_is_optional_choose_the_route_th_7ee2133e:
      "Unterstützung ist freiwillig. Wähle die Möglichkeit, die am besten zu deiner Unterstützung dieses unabhängigen Projekts passt.",
    k_support_oflh: "OFLH unterstützen",
    k_trade_off: "Zu beachten:",
  },
  table: {
    k_auto_fit_hint: "Doppelklicken, um die Spalte an den Inhalt anzupassen",
    k_fit_all_columns: "Alle Spalten anpassen",
    k_fit_all_columns_hint:
      "Alle sichtbaren Spalten an alle passenden Zeilen anpassen",
    k_columns: "Spalten",
    k_deleted: "gelöscht",
    k_file_was_deleted: "Datei wurde gelöscht",
    k_focus_mode_hint:
      "Fokusmodus: Die Seitenleiste ist eingeklappt und zusätzliche Bereiche sind ausgeblendet. {{shortcut}} drücken, um das vollständige Layout wiederherzustellen.",
    k_loading_results: "Ergebnisse werden geladen",
    k_maximize_grid: "Tabelle maximieren",
    k_no_matching_local_ports: "Keine passenden lokalen Ports",
    k_no_matching_processes: "Keine passenden Prozesse",
    k_no_visible_process_references_this_targ_b434e8b6:
      "Kein sichtbarer Prozess greift auf dieses Ziel zu. Fehlende Berechtigungen können Nutzungen verbergen.",
    k_protocol_state: "Protokoll / Status",
    k_resize_column_column: "Größe der Spalte {{column}} ändern",
    k_restore_layout: "Layout wiederherstellen",
    k_show_in_grid: "IN TABELLE ANZEIGEN",
    k_sort_by: "Sortieren nach",
    k_visible_columns: "Sichtbare Spalten",
  },
  termination: {
    k_retry_admin: "Mit Administratorrechten erneut versuchen …",
    k_admin_recovery:
      "Zugriff verweigert. Sie können einen erneuten Versuch für diese ursprünglichen Ziele autorisieren. Administratorrechte garantieren keinen Erfolg.",
    k_admin_confirmation:
      "Das Betriebssystem fordert für jedes Ziel eine Administratorautorisierung an. Die ursprüngliche Beendigungsart bleibt erhalten. Abbrechen stoppt weitere Anfragen.",

    k_normal_termination_recovery:
      "Einige Prozesse konnten nicht normal beendet werden. Mit „Beenden erzwingen“ können Sie sie ohne Aufräumen stoppen. Nicht gespeicherte Arbeit kann verloren gehen.",
    k_force_terminate: "Beenden erzwingen …",
    k_force_terminate_7b6445b3: "Beenden erzwingen",
    k_force_terminate_parent: "Elternprozess zwangsweise beenden …",
    k_force_terminate_process: "Prozess zwangsweise beenden …",
    k_force_terminate_processes: "Prozesse zwangsweise beenden?",
    k_force_termination_stops_these_processes_9c247038:
      "Beim erzwungenen Beenden können diese Prozesse keine reguläre Bereinigung durchführen. Ungespeicherte Daten können verloren gehen.",
    k_local_bindings_do_not_prove_external_re_a5cdb1ad:
      "Lokale Bindungen belegen keine Erreichbarkeit von außen. Prozesse mit unbekanntem Besitzer können nicht beendet werden.",
    k_oflh_validates_each_captured_process_id_5e6d1cc4:
      "OFLH überprüft die Identität jedes erfassten Prozesses erneut, bevor die Anfrage gesendet wird.",
    k_process_changed_or_already_exited_no_te_9b7e90d2:
      "Prozess geändert oder bereits beendet. Es wurde keine Beendigungsanfrage gesendet.",
    k_process_exited: "Prozess beendet",
    k_protected_process_or_unavailable_identity:
      "Geschützter Prozess oder Identität nicht verfügbar.",
    k_request_sent_couldn_t_verify_exit:
      "Anfrage gesendet · Beenden konnte nicht überprüft werden",
    k_request_sent_still_running_after_1_5_seconds:
      "Anfrage gesendet · nach 1,5 Sekunden noch aktiv",
    k_request_these_processes_to_stop_unsaved_8b80fb41:
      "Fordere diese Prozesse zum Beenden auf. Ungespeicherte Daten können verloren gehen. Das Beenden wird nicht erzwungen.",
    k_stopping_a_parent_may_close_its_childre_04c26acd:
      "Das Beenden eines Elternprozesses kann dessen untergeordnete Prozesse oder deine Sitzung schließen.",
    k_terminate: "Beenden …",
    k_terminate_77517bd0: "Beenden",
    k_terminate_parent: "Elternprozess beenden …",
    k_terminate_process: "Prozess beenden …",
    k_terminate_processes: "Prozesse beenden?",
    k_termination_request_failed: "Beendigungsanfrage fehlgeschlagen",
    k_termination_results: "Ergebnisse des Beendens",
    k_unknown_cpu_and_memory_stay_unavailable_1ca3b23b:
      "Unbekannte CPU- und Speicherwerte bleiben nicht verfügbar. Fehlende Berechtigungen können Prozesse verbergen. Ein normales Beenden wird niemals automatisch erzwungen. Beim Beenden eines Prozesses können ungespeicherte Daten verloren gehen.",
  },
  themes: {
    k_system_uses_theme: "Folgt deinem Gerät · {{theme}}",
    k_applied_by_system: "Vom System verwendet",

    k_themes: "Designs",
    k_bright_and_clear: "Hell und klar",
    k_choose_a_theme_or_follow_your_system:
      "Wähle ein Design oder übernimm die Systemeinstellung.",
    k_choose_your_workspace_theme_preview_it_12e8b92b:
      "Wähle ein Design für deinen Arbeitsbereich. Du kannst es jederzeit in den Einstellungen ändern.",
    k_colors_from_the_terminal_demo: "Farben aus der Terminaldemo",
    k_follow_your_device: "Systemeinstellung übernehmen",
    k_graphite_with_blue_accents: "Graphit mit blauen Akzenten",
    k_light: "Hell",
    k_oflh_purple: "OFLH Violett",
    k_rider_dark: "Rider Dunkel",
    k_rider_inspired_charcoal_with_crisp_contrast:
      "Kohlefarben mit klarem Kontrast",
    k_system: "System",
    k_vs_code_dark: "VS Code Dunkel",
  },
  update: {
    k_new_update_available: "Neues Update verfügbar",
    k_update_recommendation:
      "Updates können Fehlerbehebungen, Stabilitätsverbesserungen und neue Funktionen enthalten. Ich empfehle, die neueste Version zu installieren. Einzelheiten findest du in den Versionshinweisen.",
    k_install_question:
      "Möchtest du dieses Update installieren? OFLH startet nach der Installation neu.",
    k_manual_download:
      "Automatische Updates sind für dieses Linux-Paket nicht verfügbar. Besuche die Download-Seite und lade die App erneut herunter, um die neueste Version für dein Betriebssystem zu erhalten.",
    k_go_to_download_page: "Zur Download-Seite",
    k_later: "Später",

    k_notification: "Ein Update ist verfügbar.",
    k_download_update: "Update herunterladen",
    k_update_action_failed:
      "Das Update konnte nicht abgeschlossen werden. Erneut versuchen.",
    k_a_newer_version_is_available: "Eine neuere Version ist verfügbar.",
    k_check_for_updates: "Nach Updates suchen",
    k_checking_for_updates: "Suche nach Updates…",
    k_download_from_the_releases_page: "Von der Releases-Seite herunterladen",
    k_downloading_update: "Update wird heruntergeladen…",
    k_install_and_restart: "Installieren und neu starten",
    k_installing_update: "Update wird installiert…",
    k_restart_to_finish_installing_the_update:
      "Starte neu, um die Installation des Updates abzuschließen.",
    k_update_available_version: "Update verfügbar: v{{version}}",
    k_update_check_failed: "Updates konnten nicht geprüft werden",
    k_update_installed_restart_oflh_to_finish:
      "Update installiert. Starte OFLH neu, um fortzufahren.",
    k_update_not_supported_on_this_package:
      "Automatische Updates sind für dieses Paket nicht verfügbar. Nutze stattdessen die Releases-Seite.",
    k_updates: "Updates",
    k_view_release_notes: "Release-Notes ansehen",
    k_you_re_up_to_date: "Du bist auf dem neuesten Stand.",
  },
};

export default de;
