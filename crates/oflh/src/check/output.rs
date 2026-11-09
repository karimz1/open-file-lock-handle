use super::{Options, Outcome};
use crate::CliError;
use oflh_core::{Process, Snapshot, Usage};
use oflh_tui::Language;
use std::{fmt::Write as _, io::Write as _, path::Path};

pub(super) fn write(
    options: &Options,
    snapshot: &Snapshot,
    target: Option<&oflh_core::Target>,
    outcome: Outcome,
) -> Result<(), CliError> {
    let mut text = if options.json {
        to_json(snapshot, target, options, outcome)
    } else {
        human_text(options, snapshot, target, outcome)
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    // A reader such as `head` may close the pipe early; the exit code still
    // reports the inspection outcome instead of a panic or an inspection failure.
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(error) if error.kind() != std::io::ErrorKind::BrokenPipe => return Err(error.into()),
        _ => {}
    }
    if !options.json {
        for warning in &snapshot.warnings {
            eprintln!("oflh: warning: {}", oflh_core::safe(warning));
        }
    }
    Ok(())
}

fn human_text(
    options: &Options,
    snapshot: &Snapshot,
    target: Option<&oflh_core::Target>,
    outcome: Outcome,
) -> String {
    let language = options.language;
    let mut text = String::new();
    let holders = snapshot.processes.len();
    let path = target.map(|target| oflh_core::safe(&oflh_core::display_path(&target.path)));
    let line = match (outcome, options.port, &path, options.ports) {
        (Outcome::Free, Some(port), _, _) => match language {
            Language::English => format!("FREE: no local binding on port {port}"),
            Language::German => format!("FREI: Keine lokale Bindung an Port {port}"),
            Language::Chinese => format!("空闲：端口 {port} 没有本地绑定"),
        },
        (Outcome::Held, Some(port), _, _) => match language {
            Language::English => format!("HELD: port {port} is bound by {holders} process(es)"),
            Language::German => {
                format!("BELEGT: Port {port} ist von {holders} Prozess(en) gebunden")
            }
            Language::Chinese => format!("已占用：{holders} 个进程绑定了端口 {port}"),
        },
        (Outcome::Free, None, None, _) => match language {
            Language::English => "FREE: no local port bindings found".into(),
            Language::German => "FREI: Keine lokalen Portbindungen gefunden".into(),
            Language::Chinese => "空闲：未发现本地端口绑定".into(),
        },
        (Outcome::Held, None, None, _) => match language {
            Language::English => format!("HELD: {holders} process(es) have local port bindings"),
            Language::German => format!("BELEGT: {holders} Prozess(e) mit lokalen Portbindungen"),
            Language::Chinese => format!("已占用：{holders} 个进程有本地端口绑定"),
        },
        (Outcome::Free, None, Some(path), true) => match language {
            Language::English => {
                format!("FREE: no processes using {path} have local port bindings")
            }
            Language::German => {
                format!("FREI: Keine Prozesse, die {path} verwenden, haben lokale Portbindungen")
            }
            Language::Chinese => format!("空闲：使用 {path} 的进程没有本地端口绑定"),
        },
        (Outcome::Held, None, Some(path), true) => match language {
            Language::English => {
                format!("HELD: {holders} process(es) using {path} have local port bindings")
            }
            Language::German => format!(
                "BELEGT: {holders} Prozess(e), die {path} verwenden, haben lokale Portbindungen"
            ),
            Language::Chinese => format!("已占用：{holders} 个使用 {path} 的进程有本地端口绑定"),
        },
        (Outcome::Free, None, Some(path), false) => match language {
            Language::English => format!("FREE: no holders found for {path}"),
            Language::German => format!("FREI: Keine Nutzer für {path} gefunden"),
            Language::Chinese => format!("空闲：未发现使用 {path} 的进程"),
        },
        (Outcome::Held, None, Some(path), false) => match language {
            Language::English => format!("HELD: {path} is used by {holders} process(es)"),
            Language::German => format!("BELEGT: {path} wird von {holders} Prozess(en) verwendet"),
            Language::Chinese => format!("已占用：{holders} 个进程正在使用 {path}"),
        },
    };
    text.push_str(&line);
    text.push('\n');
    if outcome == Outcome::Held {
        for process in snapshot.processes.iter().take(5) {
            let _ = writeln!(
                text,
                "  {} (PID {})",
                oflh_core::safe(&process.name),
                process.identity.pid
            );
        }
        if holders > 5 {
            let more = holders - 5;
            let _ = match language {
                Language::English => writeln!(text, "  +{more} more"),
                Language::German => writeln!(text, "  +{more} weitere"),
                Language::Chinese => writeln!(text, "  另有 {more} 个"),
            };
        }
    }
    text
}

fn to_json(
    snapshot: &Snapshot,
    target: Option<&oflh_core::Target>,
    options: &Options,
    outcome: Outcome,
) -> String {
    let mut json = String::from("{\"version\":1,\"target\":{");
    if let Some(port) = options.port {
        write!(json, "\"kind\":\"port\",\"port\":{port}").unwrap_or_default();
    } else {
        json.push_str("\"kind\":\"path\",\"path\":");
        push_string(
            &mut json,
            &target
                .map_or_else(
                    || std::path::PathBuf::from("."),
                    |target| target.path.clone(),
                )
                .to_string_lossy(),
        );
    }
    write!(
        json,
        "}},\"free\":{},\"processes\":[",
        outcome == Outcome::Free
    )
    .unwrap_or_default();
    for (index, process) in snapshot.processes.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_process(&mut json, process);
    }
    json.push_str("],\"warnings\":[");
    for (index, warning) in snapshot.warnings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_string(&mut json, warning);
    }
    json.push_str("]}");
    json
}

fn push_process(json: &mut String, process: &Process) {
    write!(json, "{{\"pid\":{},\"name\":", process.identity.pid).unwrap_or_default();
    push_string(json, &process.name);
    json.push_str(",\"user\":");
    push_string(json, &process.user);
    json.push_str(",\"executable\":");
    push_path(json, &process.executable);
    json.push_str(",\"usages\":[");
    for (index, usage) in process.usages.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        push_usage(json, usage);
    }
    json.push_str("],\"ports\":[");
    for (index, port) in process.ports.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            json,
            "{{\"protocol\":\"{}\",\"address\":\"{}\",\"number\":{}}}",
            port.protocol.label().to_ascii_lowercase(),
            port.address,
            port.number
        )
        .unwrap_or_default();
    }
    json.push_str("]}");
}

fn push_usage(json: &mut String, usage: &Usage) {
    json.push_str("{\"path\":");
    push_path(json, &usage.path);
    write!(
        json,
        ",\"relation\":\"{}\",\"access\":\"{}\",\"deleted\":{},\"lock\":",
        usage.relation.label(),
        usage.access.label(),
        usage.deleted
    )
    .unwrap_or_default();
    match &usage.lock {
        Some(evidence) => push_string(json, &evidence.to_string()),
        None => json.push_str("null"),
    }
    json.push('}');
}

fn push_path(json: &mut String, path: &Path) {
    push_string(json, &path.to_string_lossy());
}

/// JSON strings are UTF-8; native paths use Unicode replacement for invalid sequences.
fn push_string(json: &mut String, value: &str) {
    json.push('"');
    for character in value.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\u{08}' => json.push_str("\\b"),
            '\u{0C}' => json.push_str("\\f"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            character if character <= '\u{1F}' => {
                write!(json, "\\u{:04x}", character as u32).unwrap_or_default();
            }
            character => json.push(character),
        }
    }
    json.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Access, Identity, Port, Protocol, Relation, Usage};

    #[test]
    fn human_summaries_name_port_listings_and_localize_overflow() {
        let options = Options {
            json: false,
            language: Language::German,
            path: None,
            ports: true,
            port: None,
            follow_port_folder: true,
            ports_path_only: false,
        };
        let snapshot = Snapshot {
            processes: (1..=7)
                .map(|pid| Process {
                    identity: Identity {
                        pid,
                        ..Identity::default()
                    },
                    name: format!("p{pid}"),
                    ..Process::default()
                })
                .collect(),
            warnings: vec![],
        };
        let text = human_text(&options, &snapshot, None, Outcome::Held);
        assert!(
            text.starts_with("BELEGT: 7 Prozess(e) mit lokalen Portbindungen\n"),
            "{text}"
        );
        assert!(text.ends_with("  +2 weitere\n"), "{text}");
        let free = human_text(&options, &Snapshot::default(), None, Outcome::Free);
        assert_eq!(free, "FREI: Keine lokalen Portbindungen gefunden\n");
    }

    #[test]
    fn schema_covers_empty_holder_port_and_warning_cases() {
        let options = Options {
            json: true,
            language: Language::English,
            path: Some("fixture.txt".into()),
            ports: false,
            port: None,
            follow_port_folder: false,
            ports_path_only: false,
        };
        let target = oflh_core::Target::new("fixture.txt").unwrap();
        let empty = to_json(&Snapshot::default(), Some(&target), &options, Outcome::Free);
        let mut encoded_path = String::new();
        push_string(&mut encoded_path, &target.path.to_string_lossy());
        assert_eq!(
            empty,
            format!(
                "{{\"version\":1,\"target\":{{\"kind\":\"path\",\"path\":{encoded_path}}},\"free\":true,\"processes\":[],\"warnings\":[]}}"
            )
        );

        let snapshot = Snapshot {
            processes: vec![Process {
                identity: Identity {
                    pid: 1234,
                    ..Identity::default()
                },
                name: "worker".into(),
                user: "karim".into(),
                executable: "/bin/worker".into(),
                usages: vec![Usage {
                    path: target.path.clone(),
                    relation: Relation::Open,
                    access: Access::Write,
                    deleted: false,
                    lock: None,
                }],
                ports: vec![Port {
                    protocol: Protocol::Tcp,
                    address: "0.0.0.0".parse().unwrap(),
                    number: 3000,
                }],
                ..Process::default()
            }],
            warnings: vec!["partial \"coverage\"".into()],
        };
        let json = to_json(&snapshot, Some(&target), &options, Outcome::Held);
        for expected in [
            "\"version\":1",
            "\"free\":false",
            "\"pid\":1234",
            "\"name\":\"worker\"",
            "\"relation\":\"open\"",
            "\"access\":\"write\"",
            "\"deleted\":false",
            "\"lock\":null",
            "\"protocol\":\"tcp\"",
            "\"number\":3000",
            "\"partial \\\"coverage\\\"\"",
        ] {
            assert!(json.contains(expected), "missing {expected}: {json}");
        }
    }

    #[test]
    fn port_target_and_strings_have_stable_json() {
        let options = Options {
            json: true,
            language: Language::English,
            path: None,
            ports: true,
            port: Some(3000),
            follow_port_folder: true,
            ports_path_only: false,
        };
        assert_eq!(
            to_json(&Snapshot::default(), None, &options, Outcome::Free),
            "{\"version\":1,\"target\":{\"kind\":\"port\",\"port\":3000},\"free\":true,\"processes\":[],\"warnings\":[]}"
        );
        let mut escaped = String::new();
        push_string(&mut escaped, "\"\\\n\t\u{1}");
        assert_eq!(escaped, "\"\\\"\\\\\\n\\t\\u0001\"");
    }

    #[test]
    fn lock_evidence_is_text_or_null_without_machine_unstable_structure() {
        let mut usage = Usage::default();
        let mut json = String::new();
        push_usage(&mut json, &usage);
        assert!(json.ends_with("\"lock\":null}"));
        usage.lock = Some(oflh_core::LockEvidence::SharingConflict(
            oflh_core::AccessKind::Delete,
        ));
        json.clear();
        push_usage(&mut json, &usage);
        assert!(json.contains("SHARING CONFLICT: delete denied"));
    }
}
