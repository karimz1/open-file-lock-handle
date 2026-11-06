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
    if options.json {
        let document = to_json(snapshot, target, options, outcome);
        let stdout = std::io::stdout();
        let mut stdout = stdout.lock();
        stdout.write_all(document.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    } else {
        print_human(options, snapshot, target, outcome);
    }
    Ok(())
}

fn print_human(
    options: &Options,
    snapshot: &Snapshot,
    target: Option<&oflh_core::Target>,
    outcome: Outcome,
) {
    let language = options.language;
    let target_description = if let Some(port) = options.port {
        format!("port {port}")
    } else if let Some(target) = target {
        oflh_core::safe(&oflh_core::display_path(&target.path))
    } else {
        match language {
            Language::English => "local ports".into(),
            Language::German => "lokale Ports".into(),
            Language::Chinese => "本地端口".into(),
        }
    };
    match outcome {
        Outcome::Free => match language {
            Language::English => println!("FREE: no holders found for {target_description}"),
            Language::German => println!("FREI: Keine Nutzer für {target_description} gefunden"),
            Language::Chinese => println!("空闲：未发现使用 {target_description} 的进程"),
        },
        Outcome::Held => {
            let holders = snapshot.processes.len();
            match language {
                Language::English => {
                    println!("HELD: {target_description} is used by {holders} process(es)")
                }
                Language::German => println!(
                    "BELEGT: {target_description} wird von {holders} Prozess(en) verwendet"
                ),
                Language::Chinese => {
                    println!("已占用：{holders} 个进程正在使用 {target_description}")
                }
            }
            for process in snapshot.processes.iter().take(5) {
                println!(
                    "  {} (PID {})",
                    oflh_core::safe(&process.name),
                    process.identity.pid
                );
            }
            if holders > 5 {
                println!("  +{} more", holders - 5);
            }
        }
    }
    for warning in &snapshot.warnings {
        eprintln!("oflh: warning: {}", oflh_core::safe(warning));
    }
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
