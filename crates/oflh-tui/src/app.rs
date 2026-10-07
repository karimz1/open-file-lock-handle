use crate::view::messages::Language;
use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
use oflh_core::ports::{PortIndex, PortQuery};
use oflh_core::{
    search::{ProcessIndex, Query, Scratch},
    *,
};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Details,
    Help,
    Confirm,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Relevance,
    Name,
    Pid,
    Memory,
    Cpu,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub process: usize,
    pub usages: Vec<usize>,
    pub port: Option<usize>,
    score: u32,
}
#[derive(Clone, Debug)]
pub struct ActionTarget {
    pub identity: Identity,
    pub name: String,
}
#[derive(Clone, Debug)]
pub struct Tree {
    pub nodes: Vec<ActionTarget>,
    pub cursor: usize,
}
#[derive(Debug)]
pub enum Effect {
    None,
    Quit,
    Scan,
    CancelScan,
    CheckUpdate,
    FollowPort(Identity, std::path::PathBuf),
    Kill(Vec<Identity>, bool),
    Link(&'static str),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateFeedback {
    #[default]
    Quiet,
    Checking,
    Current,
    Failed(String),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateNotice {
    pub enabled: bool,
    pub checking: bool,
    pub version: Option<String>,
    dismissed: Option<String>,
    manual: bool,
    pub feedback: UpdateFeedback,
}
impl UpdateNotice {
    pub fn visible_version(&self) -> Option<&str> {
        self.version
            .as_deref()
            .filter(|version| self.dismissed.as_deref() != Some(*version))
    }
    pub fn begin(&mut self, manual: bool) -> bool {
        if !self.enabled || self.checking {
            return false;
        }
        self.checking = true;
        self.manual = manual;
        if manual {
            self.feedback = UpdateFeedback::Checking;
        }
        true
    }
    pub fn complete(&mut self, result: std::result::Result<Option<String>, String>) -> bool {
        let before = (
            self.visible_version().map(str::to_owned),
            self.feedback.clone(),
        );
        self.checking = false;
        match result {
            Ok(Some(version)) => {
                self.version = Some(version);
                self.feedback = UpdateFeedback::Quiet;
            }
            Ok(None) if self.manual => self.feedback = UpdateFeedback::Current,
            Err(error) if self.manual => self.feedback = UpdateFeedback::Failed(error),
            _ => {}
        }
        self.manual = false;
        before
            != (
                self.visible_version().map(str::to_owned),
                self.feedback.clone(),
            )
    }
    pub fn dismiss(&mut self) {
        self.dismissed = self.version.clone();
        self.feedback = UpdateFeedback::Quiet;
    }
}
/// Immutable search data prepared by the scanner, never by the render loop.
pub struct PreparedSnapshot {
    snapshot: Snapshot,
    indices: Vec<ProcessIndex>,
    port_indices: Vec<Vec<PortIndex>>,
    positions: HashMap<Identity, usize>,
    name_keys: Vec<String>,
    file_processes: usize,
}
impl PreparedSnapshot {
    pub fn new(snapshot: Snapshot, cancel: &Cancellation) -> Result<Self> {
        cancel.set_phase(InspectionPhase::Indexing);
        let mut indices = Vec::with_capacity(snapshot.processes.len());
        let mut port_indices = Vec::with_capacity(snapshot.processes.len());
        let mut positions = HashMap::with_capacity(snapshot.processes.len());
        let mut name_keys = Vec::with_capacity(snapshot.processes.len());
        let mut file_processes = 0;
        for (position, process) in snapshot.processes.iter().enumerate() {
            cancel.check()?;
            indices.push(ProcessIndex::with_cancellation(process, cancel)?);
            port_indices.push(
                process
                    .ports
                    .iter()
                    .map(|port| PortIndex::new(process, port))
                    .collect(),
            );
            positions.insert(process.identity, position);
            name_keys.push(process.name.to_lowercase());
            file_processes += usize::from(!process.usages.is_empty());
        }
        cancel.check()?;
        Ok(Self {
            snapshot,
            indices,
            port_indices,
            positions,
            name_keys,
            file_processes,
        })
    }
}
pub struct App {
    pub target: Target,
    pub follow_port_folder: bool,
    pub version: String,
    pub language: Language,
    pub update: UpdateNotice,
    pub snapshot: Snapshot,
    indices: Vec<ProcessIndex>,
    port_indices: Vec<Vec<PortIndex>>,
    positions: HashMap<Identity, usize>,
    name_keys: Vec<String>,
    pub file_processes: usize,
    pub locked_files: usize,
    pub detail_locked_files: usize,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub screen: Screen,
    pub locked: bool,
    pub ports: bool,
    pub ports_requested: bool,
    pub ports_path_only: bool,
    pub port_query: String,
    pub detail_ports: bool,
    pub sort: Sort,
    pub selected: HashSet<Identity>,
    pub query: String,
    pub detail_query: String,
    pub editing: bool,
    edit_before: String,
    pub input_cursor: usize,
    pub detail_id: Option<Identity>,
    pub detail_scope: Option<Usage>,
    pub detail_locks: bool,
    pub usage_rows: Vec<usize>,
    pub usage_cursor: usize,
    pub path_page: usize,
    pub tree: Option<Tree>,
    pub hide_inspector: bool,
    pub auto: bool,
    pub scanning: bool,
    pub scan_started: Option<std::time::Instant>,
    pub scan_elapsed: std::time::Duration,
    pub last_scan_elapsed: Option<std::time::Duration>,
    pub scan_progress: InspectionProgress,
    pub stopping: bool,
    pub pulse: usize,
    pub pending: Vec<ActionTarget>,
    pub force: bool,
    pub confirm: bool,
    pub parent_action: bool,
    pub scroll: usize,
    pub status: String,
    pub error: bool,
    pub width: u16,
    pub height: u16,
    pub page: usize,
}
impl App {
    pub fn new(target: Target, version: String) -> Self {
        Self {
            target,
            follow_port_folder: false,
            version,
            language: Language::English,
            update: UpdateNotice::default(),
            snapshot: Snapshot::default(),
            indices: Vec::new(),
            port_indices: Vec::new(),
            positions: HashMap::new(),
            name_keys: Vec::new(),
            file_processes: 0,
            locked_files: 0,
            detail_locked_files: 0,
            rows: Vec::new(),
            cursor: 0,
            screen: Screen::Main,
            locked: false,
            ports: false,
            ports_requested: false,
            ports_path_only: false,
            port_query: String::new(),
            detail_ports: false,
            sort: Sort::Relevance,
            selected: HashSet::new(),
            query: String::new(),
            detail_query: String::new(),
            editing: false,
            edit_before: String::new(),
            input_cursor: 0,
            detail_id: None,
            detail_scope: None,
            detail_locks: false,
            usage_rows: Vec::new(),
            usage_cursor: 0,
            path_page: 0,
            tree: None,
            hide_inspector: false,
            auto: false,
            scanning: false,
            scan_started: None,
            scan_elapsed: std::time::Duration::ZERO,
            last_scan_elapsed: None,
            scan_progress: InspectionProgress::default(),
            stopping: false,
            pulse: 0,
            pending: Vec::new(),
            force: false,
            confirm: false,
            parent_action: false,
            scroll: 0,
            status: String::new(),
            error: false,
            width: 80,
            height: 24,
            page: 10,
        }
    }
    pub fn can_auto_scan(&self) -> bool {
        !self.scanning
            && !self.stopping
            && !self.editing
            && self.tree.is_none()
            && matches!(self.screen, Screen::Main | Screen::Details)
    }
    pub fn begin_scan(&mut self, now: std::time::Instant) {
        self.scanning = true;
        self.scan_started = Some(now);
        self.scan_elapsed = std::time::Duration::ZERO;
        self.scan_progress = InspectionProgress::default();
    }
    pub fn finish_scan(&mut self, now: std::time::Instant, success: bool) {
        self.scanning = false;
        if success && (self.error || self.status == self.language.text("Cancelling inspection…"))
        {
            self.status.clear();
            self.error = false;
        }
        if let Some(started) = self.scan_started.take() {
            self.scan_elapsed = now.saturating_duration_since(started);
            if success {
                self.last_scan_elapsed = Some(self.scan_elapsed);
            }
        }
    }
    pub fn current(&self) -> Option<&Process> {
        self.rows
            .get(self.cursor)
            .and_then(|row| self.snapshot.processes.get(row.process))
    }
    pub fn detail(&self) -> Option<&Process> {
        self.detail_id.and_then(|identity| {
            self.positions
                .get(&identity)
                .and_then(|&position| self.snapshot.processes.get(position))
        })
    }
    #[cfg(test)]
    pub fn replace(&mut self, snapshot: Snapshot) {
        self.accept(PreparedSnapshot::new(snapshot, &Cancellation::default()).unwrap());
    }
    pub fn accept(&mut self, prepared: PreparedSnapshot) {
        let selected = self.current().map(|process| process.identity);
        let selected_port = self.rows.get(self.cursor).and_then(|row| {
            row.port
                .map(|index| self.snapshot.processes[row.process].ports[index].clone())
        });
        let usage = self
            .rows
            .get(self.cursor)
            .filter(|_| self.locked)
            .and_then(|row| {
                self.snapshot.processes[row.process]
                    .usages
                    .get(row.usages[0])
            })
            .cloned();
        let detail_port = self
            .detail()
            .filter(|_| self.detail_ports)
            .and_then(|process| {
                self.usage_rows
                    .get(self.usage_cursor)
                    .and_then(|&index| process.ports.get(index))
                    .cloned()
            });
        let detail_usage = self
            .detail()
            .filter(|_| !self.detail_ports)
            .and_then(|process| {
                self.usage_rows
                    .get(self.usage_cursor)
                    .and_then(|&i| process.usages.get(i))
            })
            .cloned();
        self.snapshot = prepared.snapshot;
        self.indices = prepared.indices;
        self.port_indices = prepared.port_indices;
        self.positions = prepared.positions;
        self.name_keys = prepared.name_keys;
        self.file_processes = prepared.file_processes;
        self.selected
            .retain(|identity| self.positions.contains_key(identity));
        self.refilter();
        if let Some(position) = self.rows.iter().position(|row| {
            Some(self.snapshot.processes[row.process].identity) == selected
                && selected_port.as_ref().is_none_or(|port| {
                    row.port.is_some_and(|index| {
                        self.snapshot.processes[row.process].ports[index] == *port
                    })
                })
                && usage.as_ref().is_none_or(|usage| {
                    row.usages
                        .iter()
                        .any(|&i| self.snapshot.processes[row.process].usages[i] == *usage)
                })
        }) {
            self.cursor = position
        }
        self.filter_details();
        if let Some(port) = detail_port
            && let Some(process) = self.detail()
            && let Some(position) = self
                .usage_rows
                .iter()
                .position(|&index| process.ports[index] == port)
        {
            self.usage_cursor = position;
        }
        if !self.detail_ports
            && let Some(process) = self.detail()
            && let Some(position) = self
                .usage_rows
                .iter()
                .position(|&i| Some(&process.usages[i]) == detail_usage.as_ref())
        {
            self.usage_cursor = position
        }
    }
    pub fn metrics(&mut self, metrics: Vec<(Identity, Metrics)>) {
        for (identity, m) in metrics {
            if let Some(process) = self
                .positions
                .get(&identity)
                .and_then(|&position| self.snapshot.processes.get_mut(position))
            {
                process.memory = m.memory;
                process.cpu = m.cpu
            }
        }
        if matches!(self.sort, Sort::Cpu | Sort::Memory) {
            let identity = self.current().map(|process| process.identity);
            let port = self.rows.get(self.cursor).and_then(|row| row.port);
            self.sort_rows();
            if let Some(i) = self.rows.iter().position(|row| {
                Some(self.snapshot.processes[row.process].identity) == identity && row.port == port
            }) {
                self.cursor = i
            }
        }
    }
    pub fn refilter(&mut self) {
        if self.ports {
            self.filter_ports();
            return;
        }
        let query = Query::new(&self.query);
        let mut scratch = Scratch::default();
        self.rows.clear();
        let ranked = self.sort == Sort::Relevance && !query.is_empty();
        let mut locked_paths = HashSet::new();
        for (i, process) in self.snapshot.processes.iter().enumerate() {
            let index = &self.indices[i];
            // Terms already satisfied by metadata must not be recomputed for each file.
            let file_query = query.file_terms(&index.metadata, &mut scratch);
            if self.locked {
                let metadata_score = if ranked {
                    query.score(&index.metadata, &mut scratch)
                } else {
                    0
                };
                for (j, usage) in process.usages.iter().enumerate() {
                    if usage.lock.is_some() && file_query.matches(&index.usages[j], &mut scratch) {
                        locked_paths.insert(&usage.path);
                        let score = if ranked {
                            metadata_score.max(query.score(&index.usages[j], &mut scratch))
                        } else {
                            0
                        };
                        self.rows.push(Row {
                            process: i,
                            usages: vec![j],
                            port: None,
                            score,
                        });
                    }
                }
            } else {
                // Every remaining term must match one observation, never separate files.
                let usages: Vec<_> = index
                    .usages
                    .iter()
                    .enumerate()
                    .filter_map(|(j, fields)| file_query.matches(fields, &mut scratch).then_some(j))
                    .collect();
                if !usages.is_empty() {
                    let score = if ranked {
                        index.score(&query, &mut scratch)
                    } else {
                        0
                    };
                    self.rows.push(Row {
                        process: i,
                        usages,
                        port: None,
                        score,
                    });
                }
            }
        }
        self.locked_files = locked_paths.len();
        self.sort_rows();
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }
    fn sort_rows(&mut self) {
        let processes = &self.snapshot.processes;
        let sort = self.sort;
        let ports = self.ports;
        self.rows.sort_by(|a, b| {
            let left_process = &processes[a.process];
            let right_process = &processes[b.process];
            let ordering = match sort {
                Sort::Relevance if ports => {
                    a.port
                        .zip(b.port)
                        .map_or(std::cmp::Ordering::Equal, |(left, right)| {
                            left_process.ports[left]
                                .number
                                .cmp(&right_process.ports[right].number)
                        })
                }
                Sort::Relevance => b.score.cmp(&a.score),
                Sort::Name => self.name_keys[a.process].cmp(&self.name_keys[b.process]),
                Sort::Pid => left_process.identity.pid.cmp(&right_process.identity.pid),
                Sort::Memory => right_process.memory.cmp(&left_process.memory),
                Sort::Cpu => right_process
                    .cpu
                    .partial_cmp(&left_process.cpu)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            ordering.then(left_process.identity.pid.cmp(&right_process.identity.pid))
        });
    }
    pub fn filter_details(&mut self) {
        self.usage_rows.clear();
        self.detail_locked_files = 0;
        let Some(identity) = self.detail_id else {
            return;
        };
        let Some(&i) = self.positions.get(&identity) else {
            return;
        };
        let process = &self.snapshot.processes[i];
        if self.detail_ports {
            let query = PortQuery::new(&self.detail_query);
            let mut scratch = Scratch::default();
            for (index, port) in process.ports.iter().enumerate() {
                if query.matches(port, &self.port_indices[i][index], &mut scratch) {
                    self.usage_rows.push(index);
                }
            }
            self.usage_cursor = self
                .usage_cursor
                .min(self.usage_rows.len().saturating_sub(1));
            return;
        }
        let query = Query::new(&self.detail_query);
        let mut scratch = Scratch::default();
        let mut ranked = Vec::new();
        for (j, usage) in process.usages.iter().enumerate() {
            if self.detail_locks && usage.lock.is_none()
                || self
                    .detail_scope
                    .as_ref()
                    .is_some_and(|scope| scope != usage)
            {
                continue;
            }
            let fields = &self.indices[i].usages[j];
            if query.matches(fields, &mut scratch) {
                ranked.push((j, query.score(fields, &mut scratch)))
            }
        }
        self.detail_locked_files = ranked
            .iter()
            .filter_map(|&(position, _)| {
                let usage = &process.usages[position];
                usage.lock.as_ref().map(|_| &usage.path)
            })
            .collect::<HashSet<_>>()
            .len();
        ranked.sort_by_key(|&(_, text)| std::cmp::Reverse(text));
        self.usage_rows = ranked.into_iter().map(|(i, _)| i).collect();
        self.usage_cursor = self
            .usage_cursor
            .min(self.usage_rows.len().saturating_sub(1));
        self.path_page = 0;
    }
    pub fn paste(&mut self, text: &str) {
        if !self.editing {
            return;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            self.insert(c)
        }
    }
    pub fn input(&self) -> &str {
        if self.screen == Screen::Details {
            &self.detail_query
        } else if self.ports {
            &self.port_query
        } else {
            &self.query
        }
    }
    fn input_mut(&mut self) -> &mut String {
        if self.screen == Screen::Details {
            &mut self.detail_query
        } else if self.ports {
            &mut self.port_query
        } else {
            &mut self.query
        }
    }
    fn changed(&mut self) {
        if self.screen == Screen::Details {
            self.usage_cursor = 0;
            self.filter_details()
        } else {
            self.cursor = 0;
            self.refilter()
        }
    }
    fn insert(&mut self, character: char) {
        if self.input().chars().count() >= 256 {
            return;
        }
        let position = self.input_cursor;
        self.input_mut().insert(position, character);
        self.input_cursor += character.len_utf8();
        self.changed()
    }
    fn begin_search(&mut self) {
        self.editing = true;
        self.edit_before = self.input().to_owned();
        self.input_cursor = self.input().len()
    }
    fn edit_key(&mut self, key: KeyEvent) {
        match key.code {
            K::Enter => self.editing = false,
            K::Esc => {
                let before = self.edit_before.clone();
                *self.input_mut() = before;
                self.editing = false;
                self.changed()
            }
            K::Backspace => {
                let position = self.input_cursor;
                if let Some((prev, _)) = self.input()[..position].char_indices().next_back() {
                    self.input_mut().drain(prev..position);
                    self.input_cursor = prev;
                    self.changed()
                }
            }
            K::Delete => {
                let position = self.input_cursor;
                if let Some(character) = self.input()[position..].chars().next() {
                    self.input_mut()
                        .drain(position..position + character.len_utf8());
                    self.changed()
                }
            }
            K::Left => {
                self.input_cursor = self.input()[..self.input_cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i)
            }
            K::Right => {
                if let Some(character) = self.input()[self.input_cursor..].chars().next() {
                    self.input_cursor += character.len_utf8()
                }
            }
            K::Home => self.input_cursor = 0,
            K::End => self.input_cursor = self.input().len(),
            K::Up | K::Down => self.navigate(key.code),
            K::Char('u') if key.modifiers.contains(M::CONTROL) => {
                self.input_mut().clear();
                self.input_cursor = 0;
                self.changed()
            }
            K::Char(character) if !key.modifiers.intersects(M::CONTROL | M::ALT) => {
                self.insert(character)
            }
            _ => {}
        }
    }

    fn help_key(&mut self, key: K) -> Effect {
        match key {
            K::Esc | K::Char('q') | K::Char('?') => {
                self.screen = Screen::Main;
                self.scroll = 0
            }
            K::Up => self.scroll = self.scroll.saturating_sub(1),
            K::Down | K::Char('j') => self.scroll += 1,
            K::PageDown => self.scroll += self.page,
            K::PageUp => self.scroll = self.scroll.saturating_sub(self.page),
            K::Char('R') => {
                return Effect::Link("https://github.com/karimz1/open-file-lock-handle");
            }
            K::Char('D') => return Effect::Link("https://buymeacoffee.com/karimz1"),
            _ => {}
        }
        Effect::None
    }

    pub fn key(&mut self, key: KeyEvent) -> Effect {
        if key.modifiers.contains(M::CONTROL) && key.code == K::Char('c') {
            return Effect::Quit;
        }
        if self.editing {
            self.edit_key(key);
            return Effect::None;
        }
        if key.code == K::Char('q') {
            return Effect::Quit;
        }
        if key.code == K::Char('z') && self.scanning {
            return Effect::CancelScan;
        }
        if self.screen == Screen::Confirm {
            return self.confirm_key(key.code);
        }
        if self.update.enabled && !key.modifiers.intersects(M::CONTROL | M::ALT) {
            match key.code {
                K::Char('u') => return Effect::CheckUpdate,
                K::Char('U') if self.update.visible_version().is_some() => {
                    return Effect::Link(oflh_platform::updates::RELEASE_PAGE);
                }
                K::Char('b') => {
                    self.update.dismiss();
                    return Effect::None;
                }
                _ => {}
            }
        }
        if self.screen == Screen::Help {
            return self.help_key(key.code);
        }
        if let Some(tree) = &mut self.tree {
            match key.code {
                K::Esc | K::Left | K::Tab | K::BackTab | K::Char('i') => self.tree = None,
                K::Up => tree.cursor = tree.cursor.saturating_sub(1),
                K::Down | K::Char('j') => {
                    tree.cursor = (tree.cursor + 1).min(tree.nodes.len().saturating_sub(1))
                }
                K::Home => tree.cursor = 0,
                K::End => tree.cursor = tree.nodes.len().saturating_sub(1),
                K::Char('k' | 'x') => {
                    if self.width < 38 || self.height < 22 {
                        self.status = self
                            .language
                            .text("Enlarge the terminal to review the selected ancestor.")
                            .into();
                        self.error = true
                    } else {
                        self.prepare_kill(key.code)
                    }
                }
                K::Char('1' | '2' | '3') => return self.switch_tab(key.code),
                K::Char('q') => return Effect::Quit,
                _ => {}
            }
            return Effect::None;
        }
        match key.code {
            K::Char('/') => self.begin_search(),
            K::Char('r') | K::F(5) => {
                if !self.scanning && !self.stopping {
                    return Effect::Scan;
                }
            }
            K::Char('a') => self.auto = !self.auto,
            K::Char('k' | 'K' | 'x' | 'X') => self.prepare_kill(key.code),
            K::Char('?') => {
                self.screen = Screen::Help;
                self.scroll = 0
            }
            K::Char('R') => {
                return Effect::Link("https://github.com/karimz1/open-file-lock-handle");
            }
            K::Char('D') => return Effect::Link("https://buymeacoffee.com/karimz1"),
            K::Esc => {
                if self.screen == Screen::Details {
                    if self.detail_ports != self.ports {
                        self.detail_ports = self.ports;
                        self.usage_cursor = 0;
                        self.detail_query.clear();
                        self.filter_details()
                    } else {
                        self.screen = Screen::Main
                    }
                } else if !self.input().is_empty() {
                    self.input_mut().clear();
                    self.refilter()
                } else {
                    self.selected.clear()
                }
            }
            K::Up
            | K::Down
            | K::PageUp
            | K::PageDown
            | K::Home
            | K::End
            | K::Char('j' | 'g' | 'G') => self.navigate(key.code),
            _ if self.screen == Screen::Details => match key.code {
                K::Char('p' | 'f') if (key.code == K::Char('p')) != self.detail_ports => {
                    self.detail_ports = key.code == K::Char('p');
                    self.detail_query.clear();
                    self.usage_cursor = 0;
                    self.filter_details();
                    if !self.detail_ports && self.ports {
                        return self.port_folder_effect(self.detail());
                    }
                    if self.detail_ports && !self.ports_requested {
                        self.ports_requested = true;
                        if !self.stopping {
                            return Effect::Scan;
                        }
                    }
                }
                K::Char('l') if !self.detail_ports => {
                    self.detail_locks = !self.detail_locks;
                    self.filter_details()
                }
                K::Left => self.path_page = self.path_page.saturating_sub(1),
                K::Right => self.path_page = self.path_page.saturating_add(1),
                _ => {}
            },
            K::Enter => {
                self.open_details();
                if self.ports {
                    return self.port_folder_effect(self.detail());
                }
            }
            K::Char('1' | '2' | '3') => return self.switch_tab(key.code),
            K::Char('s') if self.ports => {
                self.ports_path_only = !self.ports_path_only;
                self.cursor = 0;
                self.refilter();
            }
            K::Char(' ') => {
                if let Some(identity) = self.current().map(|process| process.identity)
                    && !self.selected.remove(&identity)
                {
                    self.selected.insert(identity);
                }
            }
            K::Char('i') => self.hide_inspector = !self.hide_inspector,
            K::Tab | K::Right => {
                if let Some(process) = self.current() {
                    let mut nodes: Vec<_> = process
                        .ancestors
                        .iter()
                        .rev()
                        .map(|a| ActionTarget {
                            identity: a.identity,
                            name: a.name.clone(),
                        })
                        .collect();
                    nodes.push(ActionTarget {
                        identity: process.identity,
                        name: process.name.clone(),
                    });
                    let cursor = nodes.len() - 1;
                    self.tree = Some(Tree { nodes, cursor })
                }
            }
            K::Char('n' | 'p' | 'm' | 'c') => {
                self.sort = match key.code {
                    K::Char('n') => Sort::Name,
                    K::Char('m') => Sort::Memory,
                    K::Char('c') => Sort::Cpu,
                    _ => Sort::Pid,
                };
                self.sort_rows()
            }
            _ => {}
        }
        Effect::None
    }
    fn port_folder_effect(&self, process: Option<&Process>) -> Effect {
        if !self.follow_port_folder || self.ports_path_only || self.stopping || self.scanning {
            return Effect::None;
        }
        let Some(process) = process else {
            return Effect::None;
        };
        let folder = process.inspection_folder();
        match folder {
            Some(folder) if folder != self.target.path => {
                Effect::FollowPort(process.identity, folder.to_owned())
            }
            _ => Effect::None,
        }
    }

    fn switch_tab(&mut self, key: K) -> Effect {
        let folder_effect = if self.ports && key != K::Char('3') {
            self.port_folder_effect(
                self.rows
                    .get(self.cursor)
                    .map(|row| &self.snapshot.processes[row.process]),
            )
        } else {
            Effect::None
        };
        self.tree = None;
        self.locked = key == K::Char('2');
        self.ports = key == K::Char('3');
        self.cursor = 0;
        self.refilter();
        if !matches!(folder_effect, Effect::None) {
            return folder_effect;
        }
        if self.ports && !self.ports_requested {
            self.ports_requested = true;
            if self.stopping || self.scanning {
                Effect::None
            } else {
                Effect::Scan
            }
        } else {
            Effect::None
        }
    }

    fn filter_ports(&mut self) {
        let query = PortQuery::new(&self.port_query);
        let mut scratch = Scratch::default();
        self.rows.clear();
        for (process_index, process) in self.snapshot.processes.iter().enumerate() {
            if self.ports_path_only && process.usages.is_empty() {
                continue;
            }
            for (port_index, port) in process.ports.iter().enumerate() {
                if query.matches(
                    port,
                    &self.port_indices[process_index][port_index],
                    &mut scratch,
                ) {
                    self.rows.push(Row {
                        process: process_index,
                        usages: Vec::new(),
                        port: Some(port_index),
                        score: 0,
                    });
                }
            }
        }
        self.sort_rows();
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
    }

    pub fn select_all(&mut self) {
        let all = self.rows.iter().all(|row| {
            self.selected
                .contains(&self.snapshot.processes[row.process].identity)
        });
        for row in &self.rows {
            let identity = self.snapshot.processes[row.process].identity;
            if all {
                self.selected.remove(&identity);
            } else {
                self.selected.insert(identity);
            }
        }
    }
    fn navigate(&mut self, key: K) {
        let (len, cursor) = if self.screen == Screen::Details {
            (self.usage_rows.len(), &mut self.usage_cursor)
        } else {
            (self.rows.len(), &mut self.cursor)
        };
        *cursor = match key {
            K::Up => cursor.saturating_sub(1),
            K::Down | K::Char('j') => cursor.saturating_add(1),
            K::PageUp => cursor.saturating_sub(self.page),
            K::PageDown => cursor.saturating_add(self.page),
            K::Home | K::Char('g') => 0,
            K::End | K::Char('G') => len.saturating_sub(1),
            _ => *cursor,
        }
        .min(len.saturating_sub(1));
        self.path_page = 0;
    }
    fn open_details(&mut self) {
        let Some(row) = self.rows.get(self.cursor) else {
            return;
        };
        let process = &self.snapshot.processes[row.process];
        self.detail_id = Some(process.identity);
        self.detail_ports = self.ports;
        self.detail_scope = if self.locked {
            Some(process.usages[row.usages[0]].clone())
        } else {
            None
        };
        self.detail_query = if self.ports {
            self.port_query.clone()
        } else {
            Query::new(&self.query)
                .file_terms(&self.indices[row.process].metadata, &mut Scratch::default())
                .text()
        };
        self.detail_locks = false;
        self.usage_cursor = 0;
        self.screen = Screen::Details;
        self.filter_details()
    }
    fn prepare_kill(&mut self, key: K) {
        if self.stopping {
            return;
        }
        self.pending.clear();
        self.confirm = false;
        self.scroll = 0;
        self.force = matches!(key, K::Char('x' | 'X'));
        self.parent_action = false;
        if let Some(tree) = &self.tree {
            let target = tree.nodes[tree.cursor].clone();
            if target.identity.validate().is_err() {
                self.status = self
                    .language
                    .text("This ancestor cannot be terminated: protected or identity unavailable.")
                    .into();
                self.error = true;
                return;
            }
            self.parent_action = tree.cursor + 1 < tree.nodes.len();
            self.pending.push(target)
        } else if self.screen == Screen::Details {
            if let Some(process) = self.detail() {
                self.pending.push(ActionTarget {
                    identity: process.identity,
                    name: process.name.clone(),
                })
            }
        } else if !self.selected.is_empty() {
            self.pending.extend(
                self.snapshot
                    .processes
                    .iter()
                    .filter(|process| self.selected.contains(&process.identity))
                    .map(|process| ActionTarget {
                        identity: process.identity,
                        name: process.name.clone(),
                    }),
            )
        } else if matches!(key, K::Char('K' | 'X')) {
            let mut seen = HashSet::new();
            for row in &self.rows {
                let process = &self.snapshot.processes[row.process];
                if seen.insert(process.identity) {
                    self.pending.push(ActionTarget {
                        identity: process.identity,
                        name: process.name.clone(),
                    })
                }
            }
        } else if let Some(process) = self.current() {
            self.pending.push(ActionTarget {
                identity: process.identity,
                name: process.name.clone(),
            })
        }
        if self
            .pending
            .iter()
            .any(|target| target.identity.validate().is_err())
        {
            self.pending.clear();
            self.status = self.language.text("Cannot terminate: selection includes a protected process or an unavailable identity.").into();
            self.error = true;
            return;
        }
        if !self.pending.is_empty() {
            self.screen = Screen::Confirm
        }
    }
    fn confirm_key(&mut self, key: K) -> Effect {
        match key {
            K::Esc | K::Char('q' | 'n') => {
                self.pending.clear();
                self.screen = Screen::Main
            }
            K::Tab | K::Left | K::Right => self.confirm = !self.confirm,
            K::Up => self.scroll = self.scroll.saturating_sub(1),
            K::Down | K::Char('j') => self.scroll += 1,
            K::PageUp => self.scroll = self.scroll.saturating_sub(self.page),
            K::PageDown => self.scroll += self.page,
            K::Enter => {
                if !self.confirm {
                    self.pending.clear();
                    self.screen = Screen::Main
                } else if self.width < 38 || self.height < 22 {
                    self.status = self
                        .language
                        .text("Enlarge the terminal to review targets before confirming.")
                        .into();
                    self.error = true
                } else {
                    self.screen = Screen::Main;
                    self.stopping = true;
                    self.status = self.language.text("Requesting termination…").into();
                    return Effect::Kill(
                        self.pending
                            .drain(..)
                            .map(|process| process.identity)
                            .collect(),
                        self.force,
                    );
                }
            }
            _ => {}
        }
        Effect::None
    }
}
