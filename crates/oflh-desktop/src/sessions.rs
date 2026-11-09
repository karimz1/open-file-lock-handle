//! Owner and session lookups for rows, details and confirmations, plus the
//! single-use tickets for "Sign out user" and "Close open file".
//!
//! Like termination, an action never takes caller-supplied identities: the
//! UI only names a captured process key or a share-file id, the service
//! captures the session key (id, user, logon time) or the open (id, path,
//! user) in a ticket, and the native call runs only after a fresh read still
//! matches it.
use crate::contract::*;
use oflh_sessions::{
    Action, Availability, Os, Session, SessionKey, ShareFile, ShareFileKey, Source,
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

/// How long a session list is reused between page requests.
const SESSION_TTL: Duration = Duration::from_secs(3);

#[derive(Default)]
struct State {
    sessions: Option<(Instant, Arc<Vec<Session>>)>,
    process: HashMap<u32, Option<String>>,
    next_ticket: u64,
    sign_out: Option<(String, SessionKey)>,
    close: Option<(String, ShareFileKey)>,
}

/// Session information shared by the desktop service.
pub struct Sessions {
    source: Arc<dyn Source>,
    os: Os,
    elevated: bool,
    state: Mutex<State>,
}

fn failure(error: oflh_sessions::Error) -> Failure {
    use oflh_sessions::Error::*;
    let kind = match error {
        Unsupported => "unavailable",
        PermissionDenied => "permission_denied",
        Changed => "changed",
        Failed(_) => "io",
    };
    Failure {
        kind: kind.into(),
        message: oflh_core::safe(&error.to_string()),
        os_code: None,
        details: None,
    }
}

impl Sessions {
    /// Native sessions with this process's own elevation.
    pub fn native() -> Self {
        Self::new(
            Arc::from(oflh_sessions::native()),
            Os::current(),
            oflh_platform::elevation::is_elevated(),
        )
    }
    /// Any source, for tests and fixtures.
    pub fn new(source: Arc<dyn Source>, os: Os, elevated: bool) -> Self {
        Self {
            source,
            os,
            elevated,
            state: Mutex::default(),
        }
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
    fn table(&self) -> Arc<Vec<Session>> {
        if let Some((at, table)) = &self.lock().sessions
            && at.elapsed() < SESSION_TTL
        {
            return table.clone();
        }
        // Read outside the lock; a failed read shows no sessions, never an error row.
        let table = Arc::new(self.source.sessions().unwrap_or_default());
        let mut state = self.lock();
        state.sessions = Some((Instant::now(), table.clone()));
        state.process.clear();
        table
    }
    fn session_of(&self, pid: u32) -> Option<String> {
        if let Some(known) = self.lock().process.get(&pid) {
            return known.clone();
        }
        let id = self.source.process_session(pid);
        self.lock().process.insert(pid, id.clone());
        id
    }
    fn view(&self, session: &Session) -> SessionView {
        SessionView::new(session, self.source.own_session().as_deref())
    }
    /// Fill the session columns of visible rows only.
    pub fn annotate(&self, rows: &mut [Row]) {
        let table = self.table();
        for row in rows {
            if row.pid == 0 {
                continue;
            }
            if let Some(id) = self.session_of(row.pid) {
                row.session_kind = table
                    .iter()
                    .find(|session| session.id == id)
                    .map(|session| session.kind.as_str());
                row.session = Some(oflh_core::safe(&id));
            }
        }
    }
    fn find(&self, pid: u32) -> Option<Session> {
        let id = self.session_of(pid)?;
        self.table()
            .iter()
            .find(|session| session.id == id)
            .cloned()
    }
    /// Owner of one process. Without rights to read another user's session the
    /// user name alone is shown.
    pub fn owner(&self, user: &str, pid: u32) -> OwnerView {
        OwnerView {
            user: oflh_core::safe(user),
            service: oflh_sessions::is_service_account(user),
            session: (pid != 0)
                .then(|| self.find(pid))
                .flatten()
                .map(|session| self.view(&session)),
        }
    }
    /// Availability of session actions for a session view.
    pub fn actions(&self, session: &SessionView) -> SessionActions {
        let gate = |action| {
            oflh_sessions::availability(action, self.os, self.elevated, session.own).as_str()
        };
        SessionActions {
            send_message: gate(Action::SendMessage),
            sign_out: gate(Action::SignOut),
        }
    }
    fn gate(&self, action: Action, own: bool) -> Result<(), Failure> {
        match oflh_sessions::availability(action, self.os, self.elevated, own) {
            Availability::Available => Ok(()),
            Availability::Unsupported => Err(failure(oflh_sessions::Error::Unsupported)),
            Availability::Requires(_) => Err(failure(oflh_sessions::Error::PermissionDenied)),
        }
    }
    fn ticket(state: &mut State) -> String {
        state.next_ticket += 1;
        format!("s{}", state.next_ticket)
    }
    /// Capture the session of a process for sign-out, listing what runs in it.
    pub fn prepare_sign_out(
        &self,
        pid: u32,
        scanned: impl Iterator<Item = (u32, String)>,
    ) -> Result<SignOutConfirmation, Failure> {
        let session = self
            .find(pid)
            .ok_or_else(|| Failure::invalid("This process has no sign-in session"))?;
        let view = self.view(&session);
        self.gate(Action::SignOut, view.own)?;
        let processes = oflh_core::impact::Capped::new(
            scanned
                .filter(|(pid, _)| {
                    *pid != 0 && self.session_of(*pid).as_deref() == Some(session.id.as_str())
                })
                .map(|(pid, name)| format!("{} (PID {pid})", oflh_core::safe(&name))),
        );
        let mut state = self.lock();
        let ticket = Self::ticket(&mut state);
        state.sign_out = Some((ticket.clone(), session.key()));
        Ok(SignOutConfirmation {
            ticket,
            session: view,
            processes: processes.into(),
        })
    }
    /// Consume a sign-out ticket; the session must still match.
    pub fn sign_out(&self, ticket: &str) -> Result<(), Failure> {
        let key = {
            let mut state = self.lock();
            match state.sign_out.take() {
                Some((captured, key)) if captured == ticket => key,
                _ => {
                    return Err(Failure::invalid(
                        "Confirmation expired; review the session again",
                    ));
                }
            }
        };
        let own = self.source.own_session().as_deref() == Some(key.id.as_str());
        self.gate(Action::SignOut, own)?;
        let result = oflh_sessions::sign_out(self.source.as_ref(), &key).map_err(failure);
        self.lock().sessions = None;
        result
    }
    /// Drop outstanding session tickets.
    pub fn dismiss(&self) {
        let mut state = self.lock();
        state.sign_out = None;
        state.close = None;
    }
    /// Send a message to the session of a process; it is re-read first.
    pub fn send_message(&self, pid: u32, text: &str) -> Result<(), Failure> {
        let session = self
            .find(pid)
            .ok_or_else(|| Failure::invalid("This process has no sign-in session"))?;
        self.gate(Action::SendMessage, self.view(&session).own)?;
        oflh_sessions::send_message(
            self.source.as_ref(),
            &session.key(),
            "Open File Lock Handle",
            text,
        )
        .map_err(failure)
    }
    fn share_view(file: &ShareFile) -> ShareFileView {
        ShareFileView {
            id: file.id,
            path: oflh_core::safe(&file.path),
            user: oflh_core::safe(&file.user),
            client: file.client.as_deref().map(oflh_core::safe),
            locks: file.locks,
            write: file.write,
        }
    }
    fn matching(
        &self,
        target: &Path,
        directory: bool,
    ) -> Result<Vec<ShareFile>, oflh_sessions::Error> {
        Ok(self
            .source
            .share_files()?
            .into_iter()
            .filter(|file| file.matches(target, directory))
            .collect())
    }
    /// Share opens of the inspected file or folder.
    pub fn share_files(&self, target: Option<(&Path, bool)>) -> ShareFiles {
        let close =
            oflh_sessions::availability(Action::CloseShareFile, self.os, self.elevated, false)
                .as_str();
        if self.os != Os::Windows {
            return ShareFiles {
                close: Availability::Unsupported.as_str(),
                files: Vec::new(),
                error: None,
            };
        }
        let Some((path, directory)) = target else {
            return ShareFiles {
                close,
                files: Vec::new(),
                error: None,
            };
        };
        match self.matching(path, directory) {
            Ok(files) => ShareFiles {
                close,
                files: files.iter().take(500).map(Self::share_view).collect(),
                error: None,
            },
            Err(error) => ShareFiles {
                close,
                files: Vec::new(),
                error: Some(failure(error).kind),
            },
        }
    }
    /// Capture one share open of the target for closing.
    pub fn prepare_close_file(
        &self,
        id: u32,
        target: (&Path, bool),
    ) -> Result<CloseFileConfirmation, Failure> {
        self.gate(Action::CloseShareFile, false)?;
        let file = self
            .matching(target.0, target.1)
            .map_err(failure)?
            .into_iter()
            .find(|file| file.id == id)
            .ok_or_else(|| failure(oflh_sessions::Error::Changed))?;
        let mut state = self.lock();
        let ticket = Self::ticket(&mut state);
        state.close = Some((ticket.clone(), file.key()));
        Ok(CloseFileConfirmation {
            ticket,
            file: Self::share_view(&file),
        })
    }
    /// Consume a close-file ticket; the open must still name the same path and user.
    pub fn close_file(&self, ticket: &str) -> Result<(), Failure> {
        let key = {
            let mut state = self.lock();
            match state.close.take() {
                Some((captured, key)) if captured == ticket => key,
                _ => {
                    return Err(Failure::invalid(
                        "Confirmation expired; review the open files again",
                    ));
                }
            }
        };
        self.gate(Action::CloseShareFile, false)?;
        oflh_sessions::close_share_file(self.source.as_ref(), &key).map_err(failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_sessions::{Fake, SessionKind, SessionState};

    fn fixture(os: Os, elevated: bool) -> (Arc<Fake>, Sessions) {
        let fake = Arc::new(Fake {
            own: Some("1".into()),
            ..Fake::default()
        });
        *fake.sessions.lock().unwrap() = vec![
            Session {
                id: "1".into(),
                user: "karim".into(),
                kind: SessionKind::Console,
                state: SessionState::Active,
                since: Some(10),
                ..Session::default()
            },
            Session {
                id: "3".into(),
                user: "max.muster".into(),
                domain: Some("CORP".into()),
                kind: SessionKind::Rdp,
                state: SessionState::Active,
                client: Some("LAPTOP-7".into()),
                since: Some(20),
                ..Session::default()
            },
        ];
        fake.processes.lock().unwrap().extend([
            (100, "1".to_string()),
            (300, "3".into()),
            (301, "3".into()),
        ]);
        *fake.share_files.lock().unwrap() = vec![
            ShareFile {
                id: 7,
                path: "C:\\Share\\Q3.xlsx".into(),
                user: "anna".into(),
                client: Some("10.0.0.5".into()),
                locks: 1,
                write: true,
            },
            ShareFile {
                id: 8,
                path: "C:\\Other\\x.txt".into(),
                user: "lee".into(),
                ..ShareFile::default()
            },
        ];
        let sessions = Sessions::new(fake.clone(), os, elevated);
        (fake, sessions)
    }

    #[test]
    fn owners_and_rows_carry_session_and_service_account() {
        let (_, sessions) = fixture(Os::Windows, false);
        let owner = sessions.owner("max.muster", 300);
        let session = owner.session.clone().unwrap();
        assert_eq!(
            (
                session.id.as_str(),
                session.kind,
                session.client.as_deref(),
                session.own
            ),
            ("3", "rdp", Some("LAPTOP-7"), false)
        );
        assert!(!owner.service);
        assert!(sessions.owner("SYSTEM", 4).service);
        assert_eq!(sessions.owner("SYSTEM", 4).session, None);
        assert!(sessions.owner("karim", 100).session.unwrap().own);
        let actions = sessions.actions(&session);
        assert_eq!(
            (actions.send_message, actions.sign_out),
            ("requires_admin", "requires_admin")
        );
        let own = sessions.actions(&sessions.owner("karim", 100).session.unwrap());
        assert_eq!(own.sign_out, "available");
    }

    #[test]
    fn sign_out_ticket_lists_session_processes_and_revalidates() {
        let (fake, sessions) = fixture(Os::Windows, true);
        let scanned = vec![
            (300, "Blender".to_string()),
            (301, "Excel".into()),
            (100, "code".into()),
        ];
        let confirmation = sessions
            .prepare_sign_out(300, scanned.clone().into_iter())
            .unwrap();
        assert_eq!(
            confirmation.processes.items,
            ["Blender (PID 300)", "Excel (PID 301)"]
        );
        assert!(sessions.sign_out("wrong").is_err());
        assert!(
            sessions.sign_out(&confirmation.ticket).is_err(),
            "a wrong ticket consumes the receipt"
        );
        let confirmation = sessions.prepare_sign_out(300, scanned.into_iter()).unwrap();
        // Somebody else signed in to session 3 meanwhile: refuse.
        fake.sessions.lock().unwrap()[1].since = Some(99);
        assert_eq!(
            sessions.sign_out(&confirmation.ticket).unwrap_err().kind,
            "changed"
        );
        assert!(fake.actions().is_empty());
        fake.sessions.lock().unwrap()[1].since = Some(20);
        let confirmation = sessions.prepare_sign_out(300, std::iter::empty()).unwrap();
        sessions.sign_out(&confirmation.ticket).unwrap();
        assert!(
            sessions.sign_out(&confirmation.ticket).is_err(),
            "tickets are single use"
        );
        assert_eq!(fake.actions(), ["sign_out 3"]);
    }

    #[test]
    fn actions_are_refused_without_rights_and_never_elevate() {
        let (fake, sessions) = fixture(Os::Windows, false);
        assert_eq!(
            sessions
                .prepare_sign_out(300, std::iter::empty())
                .unwrap_err()
                .kind,
            "permission_denied"
        );
        assert_eq!(
            sessions.send_message(300, "hi").unwrap_err().kind,
            "permission_denied"
        );
        assert_eq!(
            sessions
                .prepare_close_file(7, (Path::new("C:\\Share\\Q3.xlsx"), false))
                .unwrap_err()
                .kind,
            "permission_denied"
        );
        sessions
            .send_message(100, "own session needs no admin")
            .unwrap();
        assert_eq!(fake.actions(), ["message 1 own session needs no admin"]);
        let (_, linux) = fixture(Os::Linux, false);
        let actions = linux.actions(&linux.owner("max.muster", 300).session.unwrap());
        assert_eq!(
            (actions.send_message, actions.sign_out),
            ("unsupported", "requires_root")
        );
        let (_, mac) = fixture(Os::MacOs, true);
        assert_eq!(
            mac.actions(&mac.owner("karim", 100).session.unwrap())
                .sign_out,
            "unsupported"
        );
        assert_eq!(
            mac.share_files(Some((Path::new("/x"), false))).close,
            "unsupported"
        );
    }

    #[test]
    fn share_files_match_the_target_and_close_revalidates() {
        let (fake, sessions) = fixture(Os::Windows, true);
        let target = (Path::new("c:/share"), true);
        let list = sessions.share_files(Some(target));
        assert_eq!(list.close, "available");
        assert_eq!(list.files.len(), 1);
        assert_eq!(
            (list.files[0].user.as_str(), list.files[0].client.as_deref()),
            ("anna", Some("10.0.0.5"))
        );
        assert!(
            sessions.prepare_close_file(8, target).is_err(),
            "only opens of the target"
        );
        let confirmation = sessions.prepare_close_file(7, target).unwrap();
        fake.share_files.lock().unwrap()[0].path = "C:\\Share\\other.xlsx".into();
        assert_eq!(
            sessions.close_file(&confirmation.ticket).unwrap_err().kind,
            "changed"
        );
        fake.share_files.lock().unwrap()[0].path = "C:\\Share\\Q3.xlsx".into();
        let confirmation = sessions.prepare_close_file(7, target).unwrap();
        sessions.close_file(&confirmation.ticket).unwrap();
        assert_eq!(fake.actions(), ["close 7"]);
        assert!(sessions.share_files(Some(target)).files.is_empty());
    }

    #[test]
    fn rows_get_session_columns_lazily() {
        let (_, sessions) = fixture(Os::Linux, false);
        let mut rows = vec![row(300), row(999), row(0)];
        sessions.annotate(&mut rows);
        assert_eq!(
            (rows[0].session.as_deref(), rows[0].session_kind),
            (Some("3"), Some("rdp"))
        );
        assert_eq!(rows[1].session, None);
        assert_eq!(rows[2].session, None);
    }

    fn row(pid: u32) -> Row {
        let dataset = crate::dataset::Dataset::new(
            1,
            oflh_core::Snapshot {
                processes: vec![oflh_core::Process {
                    identity: oflh_core::Identity {
                        pid,
                        started: 1,
                        started_sub: 0,
                    },
                    name: "p".into(),
                    usages: vec![oflh_core::Usage {
                        path: "/tmp/x".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                warnings: vec![],
            },
        );
        let mut page = dataset
            .page(&TableQuery {
                limit: 10,
                ..TableQuery::default()
            })
            .unwrap();
        page.rows.remove(0)
    }
}
