use crate::app::*;
#[path = "messages.rs"]
pub(crate) mod messages;
use messages::localized;
#[path = "scroll.rs"]
mod scroll;
use oflh_core::*;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Paragraph, Row as TableRow, Table, TableState, Wrap,
    },
};
use unicode_width::UnicodeWidthChar;
pub const ACCENT: Color = Color::Rgb(167, 139, 250);
pub const MUTED: Color = Color::DarkGray;
pub const LOCK: Color = Color::Rgb(229, 140, 140);
fn accent() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}
fn selected() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Rgb(109, 40, 217))
        .add_modifier(Modifier::BOLD)
}
fn access_style(access: Access) -> Style {
    Style::default().fg(match access {
        Access::Read => Color::Rgb(155, 197, 161),
        Access::Write | Access::ReadWrite => Color::Rgb(217, 183, 125),
        Access::Execute | Access::Mapped => ACCENT,
        Access::Directory | Access::Reference => Color::Rgb(154, 174, 196),
        _ => MUTED,
    })
}
pub fn cpu(process: &Process) -> String {
    process
        .cpu
        .map_or_else(|| "—".into(), |value| format!("{value:.1}%"))
}
pub fn memory(process: &Process) -> String {
    process.memory.map_or_else(
        || "—".into(),
        |value| {
            if value >= 1 << 30 {
                format!("{:.1} GiB", value as f64 / (1u64 << 30) as f64)
            } else {
                format!("{:.1} MiB", value as f64 / (1u64 << 20) as f64)
            }
        },
    )
}
fn text(frame: &mut Frame, area: Rect, value: impl Into<String>, style: Style) {
    frame.render_widget(Paragraph::new(value.into()).style(style), area)
}
fn line_area(area: Rect, y: u16) -> Rect {
    Rect::new(area.x, area.y.saturating_add(y), area.width, 1)
}
fn process_access(process: &Process, usages: &[usize]) -> Access {
    let mut read = false;
    let mut write = false;
    for &i in usages {
        match process.usages[i].access {
            Access::Read => read = true,
            Access::Write => write = true,
            Access::ReadWrite => {
                read = true;
                write = true
            }
            _ => {}
        }
    }
    if read && write {
        Access::ReadWrite
    } else if write {
        Access::Write
    } else if read {
        Access::Read
    } else {
        usages
            .first()
            .map_or(Access::Unknown, |&i| process.usages[i].access)
    }
}
fn search(frame: &mut Frame, area: Rect, app: &App, detail: bool) {
    let value = app.input();
    let label = if value.is_empty() {
        if (!detail && app.ports) || (detail && app.detail_ports) {
            app.language.text("/ Search port, process, address…")
        } else if detail {
            app.language
                .text("/ Search files, DLLs, paths… · * wildcard")
        } else {
            app.language
                .text("/ Search PID, process, path… · * wildcard")
        }
    } else {
        value
    };
    let label = if value.is_empty() {
        label.to_owned()
    } else {
        format!("/ {label}")
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if app.editing { ACCENT } else { MUTED }));
    let inner = block.inner(area);
    frame.render_widget(
        Paragraph::new(label)
            .style(Style::default().fg(if app.editing { Color::White } else { MUTED }))
            .block(block),
        area,
    );
    if app.editing && inner.width > 2 {
        let position = app.input_cursor.min(value.len());
        let width = value[..position]
            .chars()
            .map(|c| c.width().unwrap_or(0))
            .sum::<usize>();
        frame.set_cursor_position((
            inner.x + (width + 2).min(inner.width as usize - 1) as u16,
            inner.y,
        ));
    }
}
/// Render distinct action colors with a pointer on the focused choice.
fn confirmation_buttons(app: &App) -> Line<'static> {
    let cancel_color = Color::Rgb(155, 197, 161);
    let terminate_color = LOCK;
    let button = |label: &str, focused: bool, color: Color| {
        let style = if focused {
            Style::default()
                .fg(Color::Rgb(24, 24, 32))
                .bg(color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        Span::styled(
            format!("{} {label} ", if focused { "▶" } else { " " }),
            style,
        )
    };
    Line::from(vec![
        button(app.language.text("Cancel"), !app.confirm, cancel_color),
        Span::raw("   "),
        button(
            if app.force {
                app.language.text("Force quit")
            } else {
                app.language.text("Close")
            },
            app.confirm,
            terminate_color,
        ),
    ])
}

fn footer_lines(width: u16, app: &App) -> Vec<Line<'static>> {
    let (keys, status) = match app.screen {
        Screen::Confirm if app.confirm_scrollable => (app.language.text("Tab / ←→ choose · d details · ↑↓ PgUp/PgDn scroll · Enter confirm · Esc cancel"), ""),
        Screen::Confirm => (app.language.text("Tab / ←→ choose · d details · Enter confirm · Esc cancel"), ""),
        Screen::Details if app.detail_ports => (
            app.language.text("/ search · f files · r refresh · a auto · k stop · x force · ? help · Esc back · q quit"),
            "",
        ),
        Screen::Main if app.ports && app.tree.is_none() => (
            app.language.text("1/2/3 tabs · / search · s all/this path · Enter inspect · Space select · r refresh · a auto · k stop · x force · ? help · q quit"),
            "",
        ),
        Screen::Details => (
            app.language.text("/ search · p ports · l locks only · ↑↓ select · ←→ path · r refresh · a auto · k stop · x force · R GitHub · D Donate · Esc back · q quit"),
            "",
        ),
        Screen::Help => (app.language.text("↑↓ scroll · R GitHub · D Donate · Esc back · q quit"), ""),
        Screen::Main if app.tree.is_some() => (
            app.language.text("↑↓ process · k stop target · x force kill target · Tab/← back"),
            "",
        ),
        _ => (
            app.language.text("1/2/3 tabs · / search · ↑↓ move · Enter inspect · Space select · Ctrl+A all · Tab/→ tree · i panel · m/c RAM/CPU · a auto · r refresh · k stop · x force · ? help · R GitHub · D Donate · q quit"),
            app.language.text("Enter inspect · Space select · m RAM / c CPU / n name / p PID"),
        ),
    };
    let keys = if width < 60 {
        match app.screen {
            Screen::Main if app.ports && app.tree.is_none() => keys,
            Screen::Details if app.detail_ports => {
                app.language.text("/ search · r refresh · a auto · ? help · Esc back · q quit")
            }
            Screen::Main if app.tree.is_none() => {
                app.language.text("1/2/3 tabs · / search · Enter inspect · k stop · x force · ? help · R GitHub · D Donate · q quit")
            }
            Screen::Details => {
                app.language.text("↑↓ select · / search · p ports · l locks · r refresh · ? help · Esc back · q quit")
            }
            _ => keys,
        }
    } else {
        keys
    };
    let mut lines = Vec::new();
    // Reserve this line from startup for enabled release builds. A background
    // notice changes its text, never the grid's height, cursor or viewport.
    if app.update.enabled && app.screen != Screen::Confirm {
        let text = if let Some(version) = app.update.visible_version() {
            if width < 60 {
                localized!(
                    app.language,
                    "↑ {version} · U releases · b hide",
                    version = version
                )
            } else {
                localized!(
                    app.language,
                    "Update {version} available · U releases · b dismiss",
                    version = version
                )
            }
        } else {
            match &app.update.feedback {
                crate::app::UpdateFeedback::Checking => {
                    app.language.text("Checking updates…").into()
                }
                crate::app::UpdateFeedback::Current => app
                    .language
                    .text("No newer stable release · u check updates")
                    .into(),
                crate::app::UpdateFeedback::Failed(error) => localized!(
                    app.language,
                    "Update check failed: {error} · u retry · b dismiss",
                    error = safe(error)
                ),
                crate::app::UpdateFeedback::Quiet => app.language.text("u check updates").into(),
            }
        };
        lines.push(Line::styled(text, Style::default().fg(MUTED)));
    }
    if app.scanning {
        let phase = match app.scan_progress.phase {
            InspectionPhase::Processes => app.language.text("processes"),
            InspectionPhase::Files => app.language.text("files"),
            InspectionPhase::Ports => app.language.text("ports"),
            InspectionPhase::Indexing => app.language.text("indexing"),
        };
        lines.push(Line::styled(
            localized!(
                app.language,
                "{} {phase} · {:.1}s · {} processes / {} resources · z cancel",
                ["◐", "◓", "◑", "◒"][app.pulse % 4],
                app.scan_elapsed.as_secs_f64(),
                app.scan_progress.processes,
                app.scan_progress.resources,
                phase = phase
            ),
            accent(),
        ));
    } else if let Some(elapsed) = app.last_scan_elapsed {
        lines.push(Line::styled(
            localized!(app.language, "Last scan {:.2}s", elapsed.as_secs_f64()),
            Style::default().fg(MUTED),
        ));
    }
    if !app.status.is_empty() {
        lines.push(Line::styled(
            safe(&app.status),
            Style::default().fg(if app.error { LOCK } else { MUTED }),
        ))
    } else if !status.is_empty() {
        lines.push(Line::styled(status, Style::default().fg(MUTED)))
    }
    if !app.snapshot.warnings.is_empty() && app.screen != Screen::Help {
        lines.push(Line::styled(
            app.language.text("Results may be incomplete · ? details"),
            Style::default().fg(MUTED),
        ))
    }
    lines.push(Line::styled(
        "─".repeat(width as usize),
        Style::default().fg(MUTED),
    ));
    if app.screen == Screen::Confirm {
        lines.push(confirmation_buttons(app));
    }
    // Wrap whole shortcut phrases to retain meaning at narrow widths.
    let mut row = String::new();
    for hint in keys.split(" · ") {
        let sep = if row.is_empty() { "" } else { " · " };
        if unicode_width::UnicodeWidthStr::width(format!("{row}{sep}{hint}").as_str())
            > width as usize
            && !row.is_empty()
        {
            lines.push(Line::styled(std::mem::take(&mut row), accent()))
        }
        if !row.is_empty() {
            row.push_str(" · ")
        }
        row.push_str(hint)
    }
    if !row.is_empty() {
        lines.push(Line::styled(row, accent()))
    }
    if app.editing {
        lines = vec![Line::styled(
            app.language.text("Enter apply · Esc cancel · ↑↓ browse"),
            accent(),
        )]
    }
    lines
}
fn footer(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(Paragraph::new(footer_lines(area.width, app)), area);
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let full = frame.area();
    app.width = full.width;
    app.height = full.height;
    let margin = if full.width >= 10 { 2 } else { 0 };
    let area = Rect::new(
        full.x + margin,
        full.y,
        full.width.saturating_sub(margin * 2),
        full.height,
    );
    if area.width == 0 || area.height == 0 {
        return;
    }
    if area.height < 12 || area.width < 20 {
        text(
            frame,
            area,
            localized!(
                app.language,
                "oflh {}\n{}\nResize for full view · Esc back · q quit",
                display_path(&app.target.path),
                if app.screen == Screen::Confirm {
                    localized!(
                        app.language,
                        "{} targets. Enlarge to review.",
                        app.pending.len()
                    )
                } else {
                    app.current().map_or_else(String::new, |process| {
                        format!("{} {}", process.identity.pid, safe(&process.name))
                    })
                }
            ),
            accent(),
        );
        return;
    }
    match app.screen {
        Screen::Main => main_view(frame, area, app),
        Screen::Details => details(frame, area, app),
        Screen::Help | Screen::Confirm => dialog(frame, area, app),
    }
}
fn main_view(frame: &mut Frame, area: Rect, app: &mut App) {
    let full_tab_width = [" 1 Processes ", " 2 Locked files ", " 3 Ports "]
        .iter()
        .map(|label| unicode_width::UnicodeWidthStr::width(app.language.text(label)))
        .sum::<usize>()
        + 6;
    let compact_tabs = usize::from(area.width) < 48.max(full_tab_width);
    let tabs = vec![
        Span::styled("oflh  ", accent()),
        Span::styled(
            if compact_tabs {
                app.language.text(" 1 Proc ")
            } else {
                app.language.text(" 1 Processes ")
            },
            if !app.locked && !app.ports {
                selected()
            } else {
                Style::default()
                    .fg(Color::Rgb(184, 184, 204))
                    .bg(Color::Rgb(48, 48, 64))
            },
        ),
        Span::styled(
            if compact_tabs {
                app.language.text(" 2 Locks ")
            } else {
                app.language.text(" 2 Locked files ")
            },
            if app.locked {
                selected()
            } else {
                Style::default()
                    .fg(Color::Rgb(184, 184, 204))
                    .bg(Color::Rgb(48, 48, 64))
            },
        ),
        Span::styled(
            app.language.text(" 3 Ports "),
            if app.ports {
                selected()
            } else {
                Style::default()
                    .fg(Color::Rgb(184, 184, 204))
                    .bg(Color::Rgb(48, 48, 64))
            },
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(tabs)), line_area(area, 0));
    if area.width > 60 {
        text(
            frame,
            Rect::new(
                area.right().saturating_sub(app.version.len() as u16),
                area.y,
                app.version.len() as u16,
                1,
            ),
            safe(&app.version),
            Style::default().fg(MUTED),
        );
    }
    let activity = if app.stopping {
        app.language.text("requesting termination…").to_owned()
    } else if app.scanning {
        localized!(
            app.language,
            "{} scanning",
            ["◐", "◓", "◑", "◒"][app.pulse % 4]
        )
    } else if app.auto {
        app.language.text("LIVE · every 5s").into()
    } else {
        app.language.text("MANUAL · r refresh").into()
    };
    text(
        frame,
        line_area(area, 1),
        format!("{}  ·  {activity}", display_path(&app.target.path)),
        Style::default().fg(MUTED),
    );
    text(
        frame,
        line_area(area, 2),
        if app.editing {
            app.language.text("Search · Enter apply · Esc cancel")
        } else if app.tree.is_some() {
            app.language
                .text("Tree · ↑↓ choose process · k stop / x force kill · Tab/← back")
        } else {
            app.language.text("Navigation · / search · Tab/→ tree")
        },
        Style::default().fg(MUTED),
    );
    search(
        frame,
        Rect::new(area.x, area.y + 3, area.width, 3),
        app,
        false,
    );
    let summary = if app.ports {
        localized!(
            app.language,
            "{} bindings · {} · s scope",
            app.rows.len(),
            if app.ports_path_only {
                app.language.text("THIS PATH")
            } else {
                app.language.text("ALL PORTS")
            }
        )
    } else if app.locked {
        localized!(
            app.language,
            "{} locked files · {} lock entries",
            app.locked_files,
            app.rows.len()
        )
    } else {
        localized!(
            app.language,
            "{} of {} processes",
            app.rows.len(),
            app.file_processes
        )
    };
    text(
        frame,
        line_area(area, 6),
        if app.ports {
            format!(
                "{summary}{}",
                if app.selected.is_empty() {
                    String::new()
                } else {
                    localized!(app.language, " · {} selected", app.selected.len())
                }
            )
        } else {
            localized!(
                app.language,
                "{summary}{} · sort: {}",
                if app.selected.is_empty() {
                    String::new()
                } else {
                    localized!(app.language, " · {} selected", app.selected.len())
                },
                match app.sort {
                    Sort::Name => app.language.text("name"),
                    Sort::Pid => app.language.text("pid"),
                    Sort::Memory => "RAM",
                    Sort::Cpu => "CPU",
                    Sort::Relevance =>
                        if app.ports {
                            app.language.text("port")
                        } else if app.query.is_empty() {
                            app.language.text("pid")
                        } else {
                            app.language.text("match")
                        },
                },
                summary = summary
            )
        },
        accent(),
    );
    let footer_h = (footer_lines(area.width, app).len() as u16).min(area.height.saturating_sub(9));
    let body_h = area.height.saturating_sub(7 + footer_h);
    let body = Rect::new(area.x, area.y + 7, area.width, body_h);
    app.page = body_h.saturating_sub(1).max(1) as usize;
    let show =
        app.tree.is_some() || (!app.hide_inspector && area.width >= 116 && area.height >= 22);
    let panel_w = if show {
        if app.tree.is_some() && area.width < 116 {
            area.width
        } else {
            46.min(area.width / 3)
        }
    } else {
        0
    };
    let table_w = if panel_w == area.width {
        0
    } else {
        area.width
            .saturating_sub(if panel_w > 0 { panel_w + 3 } else { 0 })
    };
    if table_w > 0 {
        let table_area = Rect::new(body.x, body.y, table_w, body.height);
        render_main_table(frame, table_area, app)
    }
    if panel_w > 0 {
        inspector(
            frame,
            Rect::new(body.right() - panel_w, body.y, panel_w, body.height),
            app,
        )
    }
    footer(
        frame,
        Rect::new(area.x, area.bottom() - footer_h, area.width, footer_h),
        app,
    );
}
fn render_main_table(frame: &mut Frame, area: Rect, app: &App) {
    if app.ports {
        return port_table(frame, area, app, false);
    }
    if app.locked {
        return locked_table(frame, area, app);
    }
    if app.rows.is_empty() {
        text(
            frame,
            area,
            if app.scanning && app.snapshot.processes.is_empty() {
                app.language
                    .text("Discovering processes…\nYou can keep navigating while the scan runs.")
            } else if !app.query.is_empty() {
                app.language
                    .text("No processes match your filter.\nPress / to edit it or Esc to clear.")
            } else {
                app.language.text("No visible processes are using this path.\nPress r to scan again. Permissions may hide usage.")
            },
            Style::default().fg(MUTED),
        );
        return;
    }
    let count = area.height.saturating_sub(1).max(1) as usize;
    let start = app
        .cursor
        .saturating_sub(count - 1)
        .min(app.rows.len().saturating_sub(count));
    let wide = area.width >= 100;
    let medium = area.width >= 68;
    let show_ports = area.width >= 36;
    let widths = if wide {
        vec![
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(18),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(11),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Min(8),
        ]
    } else if medium {
        vec![
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(19),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Min(4),
        ]
    } else if show_ports {
        vec![
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Percentage(35),
            Constraint::Length(6),
            Constraint::Min(4),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Percentage(40),
            Constraint::Min(4),
        ]
    };
    let headers = if wide {
        vec![
            "",
            "PID",
            app.language.text("PROCESS"),
            app.language.text("USER"),
            "CPU%",
            "RAM",
            app.language.text("ACCESS"),
            app.language.text("PORTS"),
            app.language.text("MATCHED PATH"),
        ]
    } else if medium {
        vec![
            "",
            "PID",
            app.language.text("PROCESS"),
            app.language.text("ACCESS"),
            app.language.text("PORTS"),
            app.language.text("MATCHED PATH"),
        ]
    } else if show_ports {
        vec![
            "",
            "PID",
            app.language.text("PROCESS"),
            app.language.text("PORTS"),
            app.language.text("PATH"),
        ]
    } else {
        vec![
            "",
            "PID",
            app.language.text("PROCESS"),
            app.language.text("PATH"),
        ]
    };
    let rows = app.rows.iter().skip(start).take(count).map(|row| {
        let process = &app.snapshot.processes[row.process];
        let usage = &process.usages[row.usages[0]];
        let mark = if app.selected.contains(&process.identity) {
            "●"
        } else {
            " "
        };
        let path = if app.target.directory {
            display_path_relative_to(&usage.path, &app.target.path)
        } else {
            display_path(&usage.path)
        };
        let path = format!(
            "{}{}{}",
            path,
            if usage.deleted {
                app.language.text(" (deleted)")
            } else {
                ""
            },
            if row.usages.len() > 1 {
                format!(" +{}", row.usages.len() - 1)
            } else {
                String::new()
            }
        );
        let access = process_access(process, &row.usages);
        let mut cells = vec![
            Cell::from(mark),
            Cell::from(process.identity.pid.to_string()),
            Cell::from(safe(&process.name)),
        ];
        if wide {
            cells.extend([
                Cell::from(safe(&process.user)),
                Cell::from(cpu(process)),
                Cell::from(memory(process)),
            ]);
        }
        if medium {
            cells.push(
                Cell::from(if app.locked {
                    app.language.text("locked")
                } else {
                    app.language.text(access.label())
                })
                .style(if app.locked {
                    Style::default().fg(LOCK)
                } else {
                    access_style(access)
                }),
            )
        }
        if show_ports {
            cells.push(
                Cell::from(process.ports.len().to_string()).style(Style::default().fg(
                    if process.ports.is_empty() {
                        MUTED
                    } else {
                        ACCENT
                    },
                )),
            );
        }
        cells.push(Cell::from(path));
        TableRow::new(cells)
    });
    let table = Table::new(rows, widths)
        .header(TableRow::new(headers).style(Style::default().fg(MUTED)))
        .row_highlight_style(selected())
        .column_spacing(0);
    let mut state = TableState::default().with_selected(Some(app.cursor - start));
    frame.render_stateful_widget(table, area, &mut state);
    scroll::table(
        frame,
        area,
        app.language,
        scroll::Window {
            offset: start,
            visible: count,
            total: app.rows.len(),
        },
    );
}
fn process_port_summary(process: &Process, language: messages::Language) -> String {
    let ports: std::collections::BTreeSet<_> = process
        .ports
        .iter()
        .map(|port| (port.protocol, port.number))
        .collect();
    if ports.is_empty() {
        return language.text("none detected").into();
    }
    let mut summary = ports
        .iter()
        .take(3)
        .map(|(protocol, number)| format!("{} {number}", protocol.label()))
        .collect::<Vec<_>>()
        .join(" · ");
    if ports.len() > 3 {
        summary.push_str(&localized!(language, " · +{} more", ports.len() - 3));
    }
    summary
}

fn inspector(frame: &mut Frame, area: Rect, app: &App) {
    let captured;
    let process = if let Some(tree) = &app.tree {
        let Some(child) = tree.nodes.last() else {
            return;
        };
        captured = Process {
            identity: child.identity,
            name: child.name.clone(),
            ..Process::default()
        };
        app.snapshot
            .processes
            .iter()
            .find(|process| process.identity == child.identity)
            .unwrap_or(&captured)
    } else {
        let Some(process) = app.current() else { return };
        process
    };
    let mut lines = vec![
        Line::styled(
            format!("{} · PID {}", safe(&process.name), process.identity.pid),
            accent(),
        ),
        Line::styled("─".repeat(area.width as usize), Style::default().fg(MUTED)),
        Line::raw(format!("CPU {}   RAM {}", cpu(process), memory(process))),
        Line::styled(
            app.language.text("CPU = share of machine · RAM = RSS"),
            Style::default().fg(MUTED),
        ),
        Line::raw(""),
        Line::styled(
            if app.tree.is_some() {
                app.language.text("ANCESTRY · focused")
            } else {
                app.language.text("ANCESTRY")
            },
            accent(),
        ),
    ];
    let owned;
    let nodes = if let Some(tree) = &app.tree {
        &tree.nodes
    } else {
        owned = process
            .ancestors
            .iter()
            .rev()
            .map(|access| ActionTarget {
                identity: access.identity,
                name: access.name.clone(),
            })
            .chain(std::iter::once(ActionTarget {
                identity: process.identity,
                name: process.name.clone(),
            }))
            .collect::<Vec<_>>();
        &owned
    };
    let count = (area.height as usize)
        .saturating_sub(lines.len() + 5)
        .max(2);
    let cursor = app
        .tree
        .as_ref()
        .map_or(nodes.len().saturating_sub(1), |t| t.cursor);
    let start = cursor.saturating_sub(count.saturating_sub(2));
    let mut indices: Vec<_> = (start..nodes.len()).take(count.saturating_sub(1)).collect();
    if !indices.contains(&nodes.len().saturating_sub(1)) {
        indices.push(nodes.len().saturating_sub(1))
    }
    for i in indices {
        let node = &nodes[i];
        lines.push(Line::styled(
            format!(
                "{}└─ {} ({})",
                "  ".repeat(i.min(8)),
                safe(&node.name),
                node.identity.pid
            ),
            if app.tree.is_some() && i == cursor {
                selected()
            } else if i + 1 == nodes.len() {
                accent()
            } else {
                Style::default().fg(MUTED)
            },
        ))
    }
    if let Some(tree) = &app.tree {
        let node = &tree.nodes[tree.cursor];
        lines.extend([
            Line::raw(""),
            Line::styled(app.language.text("ACTION TARGET"), accent()),
            Line::raw(format!("{} · PID {}", safe(&node.name), node.identity.pid)),
        ]);
    }
    lines.extend([
        Line::raw(""),
        Line::styled(
            if app.tree.is_some() {
                app.language.text("PORTS")
            } else if app.ports {
                app.language.text("PORTS · Enter to inspect")
            } else {
                app.language.text("PORTS · Enter, then p to inspect")
            },
            accent(),
        ),
        Line::raw(process_port_summary(process, app.language)),
        Line::raw(""),
        Line::styled(app.language.text("EXECUTABLE"), accent()),
        Line::raw(display_path(&process.executable)),
    ]);
    if let Some(row) = app.rows.get(app.cursor)
        && let Some(usage) = row
            .usages
            .first()
            .and_then(|&index| process.usages.get(index))
    {
        lines.extend([
            Line::raw(""),
            Line::styled(app.language.text("SELECTED PATH"), accent()),
            Line::raw(display_path(&usage.path)),
        ]);
    }
    frame.render_widget(Paragraph::new(lines), area);
}
fn details(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.detail_ports {
        return port_details(frame, area, app);
    }
    text(
        frame,
        line_area(area, 0),
        app.language.text("oflh  /  process details"),
        accent(),
    );
    let Some(process) = app.detail() else {
        let footer_height = (footer_lines(area.width, app).len() as u16).min(area.height);
        text(
            frame,
            Rect::new(area.x, area.y + 2, area.width, 3),
            app.language
                .text("Process exited or PID was reused. Press Esc to return."),
            Style::default().fg(LOCK),
        );
        footer(
            frame,
            Rect::new(
                area.x,
                area.bottom() - footer_height,
                area.width,
                footer_height,
            ),
            app,
        );
        return;
    };
    let header = vec![
        Line::styled(
            format!(
                "{}   PID {} · {}",
                safe(&process.name),
                process.identity.pid,
                safe(&process.user)
            ),
            accent(),
        ),
        Line::raw(format!("EXE {}", display_path(&process.executable))),
        Line::raw(format!("CWD {}", display_path(&process.cwd))),
        Line::raw(localized!(
            app.language,
            "PARENT {}",
            process.ancestors.first().map_or_else(
                || app.language.text("unavailable").into(),
                |access| format!("{} ({})", safe(&access.name), access.identity.pid)
            )
        )),
        Line::raw(localized!(
            app.language,
            "CPU {} machine · RAM {} RSS",
            cpu(process),
            memory(process)
        )),
    ];
    let compact = area.height < 24;
    let head_height = if compact { 4 } else { 7 };
    if compact {
        let lines = vec![
            header[0].clone(),
            Line::raw(localized!(
                app.language,
                "CPU {} · RAM {} · PARENT {}",
                cpu(process),
                memory(process),
                process.parent
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines),
            Rect::new(area.x, area.y + 1, area.width, 2),
        );
    } else {
        frame.render_widget(
            Paragraph::new(header),
            Rect::new(area.x, area.y + 1, area.width, 5),
        );
    }
    text(
        frame,
        line_area(area, head_height - 1),
        localized!(
            app.language,
            "PORTS · p inspect · {}",
            process_port_summary(process, app.language)
        ),
        accent(),
    );
    search(
        frame,
        Rect::new(area.x, area.y + head_height, area.width, 3),
        app,
        true,
    );
    let locked = app.detail_locked_files;
    text(
        frame,
        line_area(area, head_height + 3),
        localized!(
            app.language,
            "{} · {} of {} usages · {locked} locked files   {} / {}",
            if app.detail_locks {
                app.language.text("LOCKS ONLY")
            } else {
                app.language.text("ALL USAGES")
            },
            app.usage_rows.len(),
            process.usages.len(),
            if app.usage_rows.is_empty() {
                0
            } else {
                app.usage_cursor + 1
            },
            app.usage_rows.len(),
            locked = locked
        ),
        accent(),
    );
    let current = app
        .usage_rows
        .get(app.usage_cursor)
        .and_then(|&i| process.usages.get(i));
    text(
        frame,
        line_area(area, head_height + 4),
        localized!(
            app.language,
            "FILE {}",
            current.map_or_else(String::new, |usage| safe(
                &usage.path.file_name().unwrap_or_default().to_string_lossy()
            ))
        ),
        Style::default().fg(MUTED),
    );
    let foot_h = (footer_lines(area.width, app).len() as u16).min(area.height.saturating_sub(9));
    let body_h = area.height.saturating_sub(head_height + 8 + foot_h);
    let body = Rect::new(area.x, area.y + head_height + 5, area.width, body_h);
    if app.usage_rows.is_empty() {
        text(
            frame,
            body,
            app.language.text("No usage entries match this filter."),
            Style::default().fg(MUTED),
        )
    } else {
        let count = body.height.saturating_sub(2).max(1) as usize;
        let start = app
            .usage_cursor
            .saturating_sub(count - 1)
            .min(app.usage_rows.len().saturating_sub(count));
        let wide = area.width >= 65;
        let widths = if wide {
            vec![
                Constraint::Percentage(35),
                Constraint::Length(14),
                Constraint::Length(13),
                Constraint::Min(4),
            ]
        } else {
            vec![Constraint::Min(8), Constraint::Length(11)]
        };
        let rows =
            app.usage_rows.iter().skip(start).take(count).map(|&i| {
                let usage = &process.usages[i];
                let mut cells = vec![Cell::from(safe(
                    &usage.path.file_name().unwrap_or_default().to_string_lossy(),
                ))];
                if wide {
                    cells.push(Cell::from(app.language.text(usage.relation.label())).style(
                        Style::default().fg(if usage.lock.is_some() { LOCK } else { ACCENT }),
                    ));
                    cells.push(
                        Cell::from(app.language.text(usage.access.label()))
                            .style(access_style(usage.access)),
                    );
                    cells.push(Cell::from(display_path(
                        usage.path.parent().unwrap_or(std::path::Path::new("")),
                    )))
                } else {
                    cells.push(Cell::from(app.language.text(usage.relation.label())))
                }
                TableRow::new(cells)
            });
        let headers = if wide {
            vec![
                app.language.text("FILE"),
                app.language.text("RELATION"),
                app.language.text("ACCESS"),
                app.language.text("DIRECTORY"),
            ]
        } else {
            vec![app.language.text("FILE"), app.language.text("RELATION")]
        };
        let table = Table::new(rows, widths)
            .column_spacing(1)
            .header(TableRow::new(headers).style(accent()))
            .row_highlight_style(selected())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(MUTED)),
            );
        let mut state = TableState::default().with_selected(Some(app.usage_cursor - start));
        frame.render_stateful_widget(table, body, &mut state);
        scroll::bordered_table(
            frame,
            body,
            app.language,
            scroll::Window {
                offset: start,
                visible: count,
                total: app.usage_rows.len(),
            },
        );
    }
    let preview_y = area.bottom().saturating_sub(foot_h + 3);
    if let Some(usage) = current {
        text(
            frame,
            Rect::new(area.x, preview_y, area.width, 1),
            localized!(
                app.language,
                "SELECTED PATH · {} · {}",
                app.language.text(usage.relation.label()),
                app.language.text(usage.access.label())
            ),
            access_style(usage.access),
        );
        let full = format!(
            "{}{}",
            display_path(&usage.path),
            usage
                .lock
                .as_ref()
                .map_or_else(String::new, |l| format!(" · {}", safe(&l.to_string())))
        );
        let page_width = area.width.max(1) as usize;
        let chars: Vec<_> = full.chars().collect();
        let position = app
            .path_page
            .min(chars.len().saturating_sub(1) / page_width)
            * page_width;
        let preview: String = chars[position..].iter().collect();
        text(
            frame,
            Rect::new(area.x, preview_y + 1, area.width, 1),
            preview,
            Style::default(),
        );
    }
    app.page = body_h.saturating_sub(2).max(1) as usize;
    footer(
        frame,
        Rect::new(area.x, area.bottom() - foot_h, area.width, foot_h),
        app,
    );
}
/// What terminating `target` may also touch: one summary line, plus the
/// capped lists when details are expanded. Silent when nothing is notable.
fn impact_lines(lines: &mut Vec<Line<'static>>, app: &App, target: &ActionTarget, width: usize) {
    const INDENT: &str = "            ";
    let language = app.language;
    let native = app.impact.get(&target.identity);
    let (files, ports) = app
        .snapshot
        .processes
        .iter()
        .find(|process| process.identity == target.identity)
        .map(impact::scanned_resources)
        .unwrap_or_default();
    let windows = native.and_then(|impact| impact.windows.as_ref());
    let children = native.and_then(|impact| impact.children.as_ref());
    // Plain consequences, most surprising first; at most two in the summary.
    let mut sentences: Vec<(String, bool)> = Vec::new();
    if native.is_some_and(|impact| impact.critical) {
        sentences.push((
            language
                .text("Windows needs this process. Ending it can crash your PC.")
                .into(),
            true,
        ))
    }
    let service = native.and_then(|impact| impact.service.as_deref());
    if service.is_some() {
        sentences.push((
            language
                .text("A service will probably restart it right away.")
                .into(),
            true,
        ))
    }
    if let Some(windows) = windows.filter(|windows| windows.len() > 1) {
        sentences.push((
            localized!(
                language,
                "{name} closes completely, with all {n} open windows, such as “{title}”.",
                name = safe(&target.name),
                n = windows.len(),
                title = safe(&windows[0])
            ),
            true,
        ))
    }
    match children.map(Vec::len) {
        Some(1) => sentences.push((
            language.text("1 background process keeps running.").into(),
            false,
        )),
        Some(n) if n > 1 => sentences.push((
            localized!(language, "{n} background processes keep running.", n = n),
            false,
        )),
        _ => {}
    }
    match files.total() {
        2 => sentences.push((
            language
                .text("1 more file it has open is released too.")
                .into(),
            false,
        )),
        n if n > 2 => sentences.push((
            localized!(
                language,
                "{n} more files it has open are released too.",
                n = n - 1
            ),
            false,
        )),
        _ => {}
    }
    if ports.total() > 1 || (ports.total() == 1 && files.total() > 0) {
        sentences.push((
            if ports.total() == 1 {
                language
                    .text("A connection on 1 network port will drop.")
                    .into()
            } else {
                localized!(
                    language,
                    "Connections on {n} network ports will drop.",
                    n = ports.total()
                )
            },
            false,
        ))
    }
    if sentences.is_empty() {
        return;
    }
    // One sentence per line, wrapped under the indent so it stays readable.
    let room = width.saturating_sub(INDENT.len()).max(16);
    for (text, warn) in sentences.into_iter().take(SUMMARY_SENTENCES) {
        let style = if warn {
            Style::default().fg(LOCK)
        } else {
            Style::default()
        };
        for row in wrap_words(&text, room) {
            lines.push(Line::from(vec![
                Span::raw(INDENT),
                Span::styled(row, style),
            ]))
        }
    }
    if !app.impact_details {
        return;
    }
    let group = |lines: &mut Vec<Line<'static>>, heading: String, rows: Vec<(String, String)>| {
        if rows.is_empty() {
            return;
        }
        lines.push(Line::styled(format!("{INDENT}{heading}"), accent()));
        for (primary, secondary) in rows {
            let mut row = vec![Span::raw(format!("{INDENT}  {primary}"))];
            if !secondary.is_empty() {
                row.push(Span::styled(
                    format!("  {secondary}"),
                    Style::default().add_modifier(Modifier::DIM),
                ))
            }
            lines.push(Line::from(row))
        }
    };
    // Each group previews a few rows; Space shows every kept row in place.
    let all = app.impact_all;
    let shorten = |mut rows: Vec<(String, String)>, unlisted: usize| {
        let total = rows.len() + unlisted;
        if total <= impact::IMPACT_PREVIEW {
            return rows;
        }
        if all {
            if unlisted > 0 {
                rows.push((
                    localized!(language, "+{n} more not listed", n = unlisted),
                    String::new(),
                ))
            }
            rows.push((language.text("▴ Space show less").into(), String::new()))
        } else {
            rows.truncate(impact::IMPACT_PREVIEW);
            rows.push((
                localized!(
                    language,
                    "▾ +{n} more · Space show all {total}",
                    n = total - impact::IMPACT_PREVIEW,
                    total = total
                ),
                String::new(),
            ))
        }
        rows
    };
    let capped = |items: Vec<(String, String)>| (items.len(), shorten(items, 0));
    let from = |list: &impact::Capped, split: fn(&str) -> (String, String)| {
        shorten(
            list.items.iter().map(|item| split(item)).collect(),
            list.more,
        )
    };
    match windows {
        Some(windows) => {
            let (total, rows) = capped(
                windows
                    .iter()
                    .map(|title| (safe(title), String::new()))
                    .collect(),
            );
            group(
                lines,
                localized!(language, "Windows ({n})", n = total),
                rows,
            )
        }
        None => lines.push(Line::styled(
            format!(
                "{INDENT}{}",
                language.text("Window list not available on this platform")
            ),
            Style::default().add_modifier(Modifier::DIM),
        )),
    }
    if let Some(children) = children.filter(|children| !children.is_empty()) {
        let (total, rows) = capped(
            children
                .iter()
                .map(|child| (safe(&child.name), format!("PID {}", child.identity.pid)))
                .collect(),
        );
        group(
            lines,
            localized!(language, "Background processes ({n})", n = total),
            rows,
        );
        lines.push(Line::styled(
            format!(
                "{INDENT}  {}",
                language.text("Background processes are not closed; some may exit with it.")
            ),
            Style::default().add_modifier(Modifier::DIM),
        ))
    }
    group(
        lines,
        localized!(language, "Files ({n})", n = files.total()),
        from(&files, |path| match path.rfind(['/', '\\']) {
            Some(cut) if cut > 0 && cut + 1 < path.len() => {
                (path[cut + 1..].to_string(), path[..cut].to_string())
            }
            _ => (path.to_string(), String::new()),
        }),
    );
    group(
        lines,
        localized!(language, "Ports ({n})", n = ports.total()),
        from(&ports, |label| {
            let (protocol, rest) = label.split_once(' ').unwrap_or(("", label));
            match rest.rsplit_once(':') {
                Some((address, port)) => (port.to_string(), format!("{protocol} {address}")),
                None => (rest.to_string(), protocol.to_string()),
            }
        }),
    );
    if let Some(service) = service {
        group(
            lines,
            language.text("Service").into(),
            vec![(safe(service), language.text("may restart it").into())],
        );
    }
}

/// Collapsed confirmation rows stay at about two lines.
const SUMMARY_SENTENCES: usize = 2;

/// Wraps at spaces, or between characters for scripts without them.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in text.split_inclusive(' ') {
        if row.width() + word.trim_end().width() > width && !row.is_empty() {
            rows.push(row.trim_end().to_string());
            row.clear();
        }
        if word.trim_end().width() > width {
            for ch in word.chars() {
                if row.width() + unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) > width
                    && !row.is_empty()
                {
                    rows.push(std::mem::take(&mut row));
                }
                row.push(ch)
            }
        } else {
            row.push_str(word)
        }
    }
    if !row.trim().is_empty() {
        rows.push(row.trim_end().to_string())
    }
    rows
}

fn dialog(frame: &mut Frame, area: Rect, app: &mut App) {
    text(
        frame,
        line_area(area, 0),
        "oflh  ·  Open File Lock Handle",
        accent(),
    );
    let mut lines = Vec::new();
    if app.screen == Screen::Confirm {
        lines.push(Line::styled(
            confirm_title(app),
            Style::default().fg(LOCK).add_modifier(Modifier::BOLD),
        ));
        let single = app.pending.len() == 1;
        lines.push(Line::raw(app.language.text(match (app.force, single) {
            (false, true) => "OFLH asks the program to close. Unsaved work may be lost.",
            (false, false) => "OFLH asks each program to close. Unsaved work may be lost.",
            (true, true) => {
                "OFLH stops it immediately, without letting it save. Unsaved work will be lost."
            }
            (true, false) => {
                "OFLH stops them immediately, without letting them save. Unsaved work will be lost."
            }
        })));
        if app.parent_action {
            lines.push(Line::styled(
                app.language.text(
                    "Target: selected ancestor. Its application and children may be affected.",
                ),
                Style::default().fg(LOCK),
            ))
        }
        lines.push(Line::raw(""));
        lines.push(Line::raw(
            app.language.text("Processes (filtered ones included):"),
        ));
        let checking = app.impact.is_empty();
        for process in &app.pending {
            lines.push(Line::raw(format!(
                "  {:<9} {}",
                process.identity.pid,
                safe(&process.name)
            )));
            impact_lines(&mut lines, app, process, area.width as usize);
        }
        if checking {
            lines.push(Line::styled(
                app.language.text("Checking windows and child processes…"),
                Style::default().add_modifier(Modifier::DIM),
            ))
        } else if app.impact_details {
            lines.push(Line::styled(
                app.language
                    .text("Files and ports come from the latest scan."),
                Style::default().add_modifier(Modifier::DIM),
            ))
        }
    } else {
        lines.push(Line::styled(app.language.text("LINKS"), accent()));
        lines.push(Line::raw(
            "R  GitHub: https://github.com/karimz1/open-file-lock-handle",
        ));
        lines.push(Line::raw(
            app.language
                .text("D  Donate: https://buymeacoffee.com/karimz1"),
        ));
        lines.push(Line::raw(
            app.language
                .text("Press uppercase R or D to open in your browser."),
        ));
        lines.push(Line::raw(""));
        if !app.snapshot.warnings.is_empty() {
            lines.push(Line::styled(app.language.text("SCAN DETAILS"), accent()));
        }
        for warning in &app.snapshot.warnings {
            lines.push(Line::raw(safe(warning)))
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            localized!(app.language, "VERSION {}", oflh_core::VERSION),
            accent(),
        ));
        if !oflh_core::BUILD_COMMIT.is_empty() {
            lines.push(Line::raw(localized!(
                app.language,
                "Commit {}",
                oflh_core::BUILD_COMMIT
            )));
        }
        if !oflh_core::BUILD_URL.is_empty() {
            lines.push(Line::raw(localized!(
                app.language,
                "Build {}",
                oflh_core::BUILD_URL
            )));
        }
        if !oflh_core::PULL_REQUEST_URL.is_empty() {
            lines.push(Line::raw(localized!(
                app.language,
                "Pull request {}",
                oflh_core::PULL_REQUEST_URL
            )));
        }
        lines.push(Line::raw(
            app.language
                .text("Run `oflh --version` to copy the full build report."),
        ));
        lines.push(Line::raw(""));
        lines.extend(app.language.help().lines().map(|s| Line::raw(s.to_owned())));
    }
    let confirm = app.screen == Screen::Confirm;
    // Size the footer for the longer scroll hint so the body height below is
    // final; the buttons in the footer are never covered by content.
    app.confirm_scrollable = confirm;
    let footer_height = (footer_lines(area.width, app).len() as u16).min(area.height);
    let height = area.height.saturating_sub(footer_height + 2);
    app.page = height.max(1) as usize;
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let mut body = Rect::new(area.x, area.y + 2, area.width, height);
    // Help and the confirmation scroll by wrapped line, with the shared
    // scrollbar, indicator and key hint while they overflow.
    let mut total = paragraph.line_count(area.width);
    if total > height as usize && area.width > 1 {
        // Leave the right column for the scrollbar and re-measure.
        body.width -= 1;
        total = paragraph.line_count(body.width);
    }
    let last = total.saturating_sub(height as usize);
    app.scroll = app.scroll.min(last);
    if confirm {
        app.confirm_scrollable = last > 0;
    }
    let window = scroll::Window {
        offset: app.scroll,
        visible: height as usize,
        total,
    };
    scroll::indicator_line(
        frame,
        line_area(area, 0),
        Some(line_area(area, 1)),
        app.language,
        window,
    );
    scroll::scrollbar(
        frame,
        Rect::new(area.x, body.y, area.width, body.height),
        window,
    );
    frame.render_widget(
        paragraph.scroll((app.scroll.min(u16::MAX as usize) as u16, 0)),
        body,
    );
    footer(
        frame,
        Rect::new(
            area.x,
            area.bottom() - footer_height,
            area.width,
            footer_height,
        ),
        app,
    );
}

fn locked_table(frame: &mut Frame, area: Rect, app: &App) {
    if app.rows.is_empty() {
        text(
            frame,
            area,
            if app.scanning {
                app.language.text("Scanning for file locks…")
            } else if !app.query.is_empty() {
                app.language.text("No locked files match your filter.")
            } else {
                app.language.text("No confirmed locks found in this scan.\nVisibility depends on permissions; press r to refresh.")
            },
            Style::default().fg(MUTED),
        );
        return;
    }
    let count = area.height.saturating_sub(1).max(1) as usize;
    let start = app
        .cursor
        .saturating_sub(count - 1)
        .min(app.rows.len().saturating_sub(count));
    let show_name = area.width >= 40;
    let mut widths = vec![Constraint::Length(3), Constraint::Length(8)];
    let mut headers = vec!["", "PID"];
    if show_name {
        widths.push(Constraint::Length(if area.width < 62 { 10 } else { 18 }));
        headers.push(app.language.text("PROCESS"));
    }
    widths.push(Constraint::Min(4));
    headers.push(app.language.text("LOCKED FILE"));
    let rows = app.rows.iter().skip(start).take(count).map(|row| {
        let process = &app.snapshot.processes[row.process];
        let usage = &process.usages[row.usages[0]];
        let mut cells = vec![
            Cell::from(if app.selected.contains(&process.identity) {
                "●"
            } else {
                " "
            }),
            Cell::from(process.identity.pid.to_string()),
        ];
        if show_name {
            cells.push(Cell::from(safe(&process.name)))
        }
        cells.push(Cell::from(format!(
            "{}{}",
            display_path(&usage.path),
            if usage.deleted {
                app.language.text(" (deleted)")
            } else {
                ""
            }
        )));
        TableRow::new(cells)
    });
    let table = Table::new(rows, widths)
        .column_spacing(0)
        .header(TableRow::new(headers).style(Style::default().fg(MUTED)))
        .row_highlight_style(selected());
    let mut state = TableState::default().with_selected(Some(app.cursor - start));
    frame.render_stateful_widget(table, area, &mut state);
    scroll::table(
        frame,
        area,
        app.language,
        scroll::Window {
            offset: start,
            visible: count,
            total: app.rows.len(),
        },
    );
}

fn port_table(frame: &mut Frame, area: Rect, app: &App, detail: bool) {
    let (length, cursor) = if detail {
        (app.usage_rows.len(), app.usage_cursor)
    } else {
        (app.rows.len(), app.cursor)
    };
    if length == 0 {
        text(
            frame,
            area,
            if app.scanning {
                app.language.text("Scanning local ports…")
            } else {
                app.language.text(
                    "No matching port bindings.
/ search · r refresh · s all/this path",
                )
            },
            Style::default().fg(MUTED),
        );
        return;
    }
    let count = area.height.saturating_sub(1).max(1) as usize;
    let start = cursor
        .saturating_sub(count - 1)
        .min(length.saturating_sub(count));
    let wide = area.width >= 80;
    let medium = area.width >= 54;
    let widths = if wide {
        vec![
            Constraint::Length(2),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(18),
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(10),
        ]
    } else if medium {
        vec![
            Constraint::Length(2),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(14),
            Constraint::Min(12),
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Min(5),
        ]
    };
    let mut headers = vec![
        "",
        app.language.text("PORT"),
        if medium {
            app.language.text("PROTO")
        } else {
            app.language.text("NET")
        },
        "PID",
        app.language.text("PROCESS"),
    ];
    if medium {
        headers.push(app.language.text("ADDRESS"));
    }
    if wide {
        headers.extend([app.language.text("STATE"), app.language.text("THIS PATH")]);
    }
    let mut rows = Vec::new();
    for position in start..(start + count).min(length) {
        let binding = if detail {
            app.detail().and_then(|process| {
                process
                    .ports
                    .get(app.usage_rows[position])
                    .map(|port| (process, port))
            })
        } else {
            let row = &app.rows[position];
            let process = &app.snapshot.processes[row.process];
            row.port
                .and_then(|index| process.ports.get(index))
                .map(|port| (process, port))
        };
        let Some((process, port)) = binding else {
            continue;
        };
        let mut cells = vec![
            Cell::from(if app.selected.contains(&process.identity) {
                "●"
            } else {
                " "
            }),
            Cell::from(port.number.to_string()),
            Cell::from(port.protocol.label()).style(access_style(
                if port.protocol == Protocol::Tcp {
                    Access::Read
                } else {
                    Access::Write
                },
            )),
            Cell::from(if process.identity.pid == 0 {
                "—".into()
            } else {
                process.identity.pid.to_string()
            }),
            Cell::from(safe(&process.name)),
        ];
        if medium {
            cells.push(Cell::from(port.address.to_string()));
        }
        if wide {
            cells.extend([
                Cell::from(app.language.text(port.protocol.state())),
                Cell::from(if process.usages.is_empty() {
                    "—"
                } else {
                    app.language.text("yes")
                }),
            ]);
        }
        rows.push(TableRow::new(cells));
    }
    let table = Table::new(rows, widths)
        .header(TableRow::new(headers).style(Style::default().fg(MUTED)))
        .column_spacing(0)
        .row_highlight_style(selected());
    let mut state = TableState::default().with_selected(Some(cursor - start));
    frame.render_stateful_widget(table, area, &mut state);
    scroll::table(
        frame,
        area,
        app.language,
        scroll::Window {
            offset: start,
            visible: count,
            total: length,
        },
    );
}

fn port_details(frame: &mut Frame, area: Rect, app: &mut App) {
    text(
        frame,
        line_area(area, 0),
        app.language.text("oflh / process details · ports"),
        accent(),
    );
    text(
        frame,
        line_area(area, 1),
        if app.auto {
            app.language.text("LIVE · every 5s")
        } else {
            app.language.text("MANUAL · r refresh")
        },
        Style::default().fg(MUTED),
    );
    let Some(process) = app.detail() else {
        text(
            frame,
            line_area(area, 3),
            app.language
                .text("Process exited or PID was reused. Esc back."),
            Style::default().fg(LOCK),
        );
        return;
    };
    text(
        frame,
        line_area(area, 2),
        format!(
            "{} · PID {}",
            safe(&process.name),
            if process.identity.pid == 0 {
                app.language.text("unavailable").into()
            } else {
                process.identity.pid.to_string()
            }
        ),
        accent(),
    );
    text(
        frame,
        line_area(area, 3),
        format!("EXE {}", display_path(&process.executable)),
        Style::default(),
    );
    text(
        frame,
        line_area(area, 4),
        format!("CWD {}", display_path(&process.cwd)),
        Style::default(),
    );
    search(
        frame,
        Rect::new(area.x, area.y + 5, area.width, 3),
        app,
        true,
    );
    text(
        frame,
        line_area(area, 8),
        localized!(
            app.language,
            "{} of {} bindings · f file usages",
            app.usage_rows.len(),
            process.ports.len()
        ),
        accent(),
    );
    let footer_height =
        (footer_lines(area.width, app).len() as u16).min(area.height.saturating_sub(11));
    let height = area.height.saturating_sub(11 + footer_height);
    port_table(
        frame,
        Rect::new(area.x, area.y + 9, area.width, height),
        app,
        true,
    );
    if let Some(port) = app
        .usage_rows
        .get(app.usage_cursor)
        .and_then(|&index| process.ports.get(index))
    {
        text(
            frame,
            line_area(area, 9 + height),
            format!(
                "{} {} · {}",
                port.protocol.label(),
                std::net::SocketAddr::new(port.address, port.number),
                app.language.text(port.protocol.state())
            ),
            accent(),
        );
    }
    app.page = height.saturating_sub(1).max(1) as usize;
    footer(
        frame,
        Rect::new(
            area.x,
            area.bottom() - footer_height,
            area.width,
            footer_height,
        ),
        app,
    );
}

/// "Close Blender?" for one target, "Close 3 processes?" for several; the
/// force variant says "Force quit" so title and button use the same verb.
fn confirm_title(app: &App) -> String {
    let n = app.pending.len();
    let name = app
        .pending
        .first()
        .map(|process| safe(&process.name).chars().take(40).collect::<String>())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            app.pending
                .first()
                .map(|process| format!("PID {}", process.identity.pid))
                .unwrap_or_default()
        });
    match (app.force, n == 1) {
        (false, true) => localized!(app.language, "Close {name}?", name = name),
        (false, false) => localized!(app.language, "Close {n} processes?", n = n),
        (true, true) => localized!(app.language, "Force quit {name}?", name = name),
        (true, false) => localized!(app.language, "Force quit {n} processes?", n = n),
    }
}
