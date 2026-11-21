//! App windows by owning PID on an X11 display, including XWayland.
//!
//! The termination preview uses this to say how many windows close with a
//! process. It follows the EWMH conventions every mainstream toolkit and
//! window manager implements, so nothing here is specific to one app:
//!
//! - with a window manager, the managed client list (`_NET_CLIENT_LIST`),
//!   which also covers minimized windows; without one, mapped top-level
//!   windows that are not override-redirect popups
//! - the owner is the PID the X server measured for the local client that
//!   created the window (X-Resource extension); when the server lacks that
//!   extension, the `_NET_WM_PID` hint, ignored for clients on another host
//! - normal windows count; docks, toolbars, utility palettes, menus, splash
//!   screens and dialogs that belong to another shown window do not
use std::collections::{HashMap, HashSet};
use x11rb::{
    connection::{Connection, RequestConnection},
    protocol::{
        res::{ClientIdMask, ClientIdSpec, ConnectionExt as _},
        xproto::{AtomEnum, ConnectionExt, MapState, Window},
    },
    rust_connection::RustConnection,
};

/// Window titles by PID, or `None` when no X display is reachable.
pub(crate) fn window_titles() -> Option<HashMap<u32, Vec<String>>> {
    std::env::var_os("DISPLAY")?;
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    Titles::new(&connection)?.collect(root)
}

/// On a Wayland session the X display only shows XWayland clients, so a
/// process without X windows may still have native Wayland windows.
pub(crate) fn complete_for_absent_processes() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_none()
        && std::env::var("XDG_SESSION_TYPE").map_or(true, |kind| kind != "wayland")
}

struct Atoms {
    client_list: u32,
    pid: u32,
    name: u32,
    utf8: u32,
    window_type: u32,
    normal: u32,
    dialog: u32,
}

struct Titles<'a> {
    connection: &'a RustConnection,
    atoms: Atoms,
    host: Option<Vec<u8>>,
    resources: bool,
}

impl<'a> Titles<'a> {
    fn new(connection: &'a RustConnection) -> Option<Self> {
        let names = [
            "_NET_CLIENT_LIST",
            "_NET_WM_PID",
            "_NET_WM_NAME",
            "UTF8_STRING",
            "_NET_WM_WINDOW_TYPE",
            "_NET_WM_WINDOW_TYPE_NORMAL",
            "_NET_WM_WINDOW_TYPE_DIALOG",
        ];
        let cookies: Vec<_> = names
            .iter()
            .map(|name| connection.intern_atom(false, name.as_bytes()).ok())
            .collect::<Option<_>>()?;
        let atoms: Vec<u32> = cookies
            .into_iter()
            .map(|cookie| cookie.reply().ok().map(|reply| reply.atom))
            .collect::<Option<_>>()?;
        Some(Self {
            connection,
            atoms: Atoms {
                client_list: atoms[0],
                pid: atoms[1],
                name: atoms[2],
                utf8: atoms[3],
                window_type: atoms[4],
                normal: atoms[5],
                dialog: atoms[6],
            },
            host: hostname(),
            resources: connection
                .extension_information(x11rb::protocol::res::X11_EXTENSION_NAME)
                .ok()
                .flatten()
                .is_some(),
        })
    }

    fn property(&self, window: Window, property: u32, kind: u32) -> Option<Vec<u8>> {
        let reply = self
            .connection
            .get_property(false, window, property, kind, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        (reply.type_ != u32::from(AtomEnum::NONE)).then_some(reply.value)
    }

    fn cardinals(&self, window: Window, property: u32, kind: impl Into<u32>) -> Vec<u32> {
        self.property(window, property, kind.into())
            .map(|bytes| {
                // x11rb negotiates native byte order for the connection.
                let (chunks, _) = bytes.as_chunks::<4>();
                chunks
                    .iter()
                    .map(|&chunk| u32::from_ne_bytes(chunk))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Managed clients when a window manager runs, otherwise mapped
    /// top-level windows that are not override-redirect popups.
    fn candidates(&self, root: Window) -> Option<Vec<Window>> {
        let managed = self.cardinals(root, self.atoms.client_list, AtomEnum::WINDOW);
        if !managed.is_empty() {
            return Some(managed);
        }
        let tree = self.connection.query_tree(root).ok()?.reply().ok()?;
        Some(
            tree.children
                .into_iter()
                .filter(|&window| {
                    self.connection
                        .get_window_attributes(window)
                        .ok()
                        .and_then(|cookie| cookie.reply().ok())
                        .is_some_and(|attributes| {
                            attributes.map_state == MapState::VIEWABLE
                                && !attributes.override_redirect
                        })
                })
                .collect(),
        )
    }

    fn title(&self, window: Window) -> Option<String> {
        let title = self
            .property(window, self.atoms.name, self.atoms.utf8)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .or_else(|| {
                self.property(window, AtomEnum::WM_NAME.into(), AtomEnum::ANY.into())
                    .map(|bytes| bytes.iter().map(|&byte| char::from(byte)).collect())
            })?;
        let title = title.trim_end_matches('\0').to_owned();
        (!title.is_empty()).then_some(title)
    }

    fn local(&self, window: Window) -> bool {
        match (
            &self.host,
            self.property(
                window,
                AtomEnum::WM_CLIENT_MACHINE.into(),
                AtomEnum::ANY.into(),
            ),
        ) {
            (Some(host), Some(machine)) => {
                let machine = machine.split(|&byte| byte == 0).next().unwrap_or_default();
                machine.eq_ignore_ascii_case(host)
                    || machine
                        .split(|&byte| byte == b'.')
                        .next()
                        .is_some_and(|short| short.eq_ignore_ascii_case(host))
            }
            _ => true,
        }
    }

    /// The PID the server measured for the local client owning `window`.
    fn measured_pid(&self, window: Window) -> Option<u32> {
        if !self.resources {
            return None;
        }
        let reply = self
            .connection
            .res_query_client_ids(&[ClientIdSpec {
                client: window,
                mask: ClientIdMask::LOCAL_CLIENT_PID,
            }])
            .ok()?
            .reply()
            .ok()?;
        reply
            .ids
            .iter()
            .find(|id| id.spec.mask == ClientIdMask::LOCAL_CLIENT_PID)
            .and_then(|id| id.value.first().copied())
            .filter(|&pid| pid != 0)
    }

    fn owner(&self, window: Window) -> Option<u32> {
        self.measured_pid(window).or_else(|| {
            self.local(window)
                .then(|| {
                    self.cardinals(window, self.atoms.pid, AtomEnum::CARDINAL)
                        .first()
                        .copied()
                })
                .flatten()
        })
    }

    fn collect(&self, root: Window) -> Option<HashMap<u32, Vec<String>>> {
        let candidates = self.candidates(root)?;
        let shown: HashSet<Window> = candidates.iter().copied().collect();
        let mut titles: HashMap<u32, Vec<String>> = HashMap::new();
        for window in candidates {
            let types = self.cardinals(window, self.atoms.window_type, AtomEnum::ATOM);
            // The first type the client lists is its preferred one; no type
            // means a normal window, or a dialog when it is transient.
            let transient = self
                .cardinals(window, AtomEnum::WM_TRANSIENT_FOR.into(), AtomEnum::WINDOW)
                .first()
                .copied()
                .filter(|&parent| parent != 0 && parent != root && shown.contains(&parent));
            let counts = match types.first() {
                None => transient.is_none(),
                Some(&kind) if kind == self.atoms.normal => true,
                Some(&kind) if kind == self.atoms.dialog => transient.is_none(),
                Some(_) => false,
            };
            if !counts {
                continue;
            }
            let Some(pid) = self.owner(window) else {
                continue;
            };
            if let Some(title) = self.title(window) {
                titles.entry(pid).or_default().push(title);
            }
        }
        Some(titles)
    }
}

fn hostname() -> Option<Vec<u8>> {
    let name = std::fs::read("/proc/sys/kernel/hostname").ok()?;
    let name: Vec<u8> = name.trim_ascii().to_vec();
    (!name.is_empty()).then_some(name)
}
