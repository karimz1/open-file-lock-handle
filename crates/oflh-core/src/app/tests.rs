//! Root finding is the guard that keeps "close the app" from reaching a
//! launcher, a shell or the session. Every rule has a case here, built from
//! the process layout seen with a real multi-process app (one executable for
//! the main process, the network, GPU and utility helpers, and zygotes).
use super::*;

const APP: &str = "/opt/Example/example";
const SHELL: &str = "/usr/bin/bash";

fn process(pid: u32, parent: u32, started: u64, executable: &str) -> Lineage {
    Lineage {
        identity: Identity {
            pid,
            started,
            started_sub: 0,
        },
        parent,
        name: Path::new(executable)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        executable: executable.into(),
        owner: Some("karim".into()),
        session: Some(1),
        ..Lineage::default()
    }
}
fn visible(mut lineage: Lineage) -> Lineage {
    lineage.visible = true;
    lineage
}

#[test]
fn a_helper_resolves_to_the_visible_main_process() {
    // Network service helper <- main window process <- launcher.
    let chain = [
        process(300, 200, 30, APP),
        visible(process(200, 100, 20, APP)),
        visible(process(100, 1, 10, "/usr/bin/plasmashell")),
    ];
    assert_eq!(app_root(&chain, Images::Exact), Some(1));
}

#[test]
fn renderers_below_a_zygote_resolve_to_the_main_process() {
    let chain = [
        process(400, 310, 40, APP),
        process(310, 200, 31, APP),
        visible(process(200, 100, 20, APP)),
        process(100, 1, 10, SHELL),
    ];
    assert_eq!(app_root(&chain, Images::Exact), Some(2));
}

#[test]
fn the_visible_app_itself_is_never_escalated() {
    let chain = [
        visible(process(300, 200, 30, APP)),
        visible(process(200, 100, 20, APP)),
    ];
    assert_eq!(app_root(&chain, Images::Exact), None);
}

#[test]
fn a_parent_with_another_executable_stops_the_walk() {
    // Launchers, shells, terminals and file managers run other executables.
    let chain = [
        process(300, 200, 30, APP),
        visible(process(200, 100, 20, "/usr/bin/gnome-terminal-server")),
    ];
    assert_eq!(app_root(&chain, Images::Exact), None);
}

#[test]
fn a_same_executable_chain_without_a_visible_process_is_not_an_app() {
    // Nested shells or a worker pool: nothing a person sees as one app.
    let chain = [
        process(300, 200, 30, SHELL),
        process(200, 100, 20, SHELL),
        visible(process(100, 1, 10, "/usr/bin/xterm")),
    ];
    assert_eq!(app_root(&chain, Images::Exact), None);
}

#[test]
fn the_topmost_visible_process_wins_but_never_past_a_break() {
    let chain = [
        process(500, 400, 50, APP),
        visible(process(400, 300, 40, APP)),
        process(300, 200, 30, "/usr/bin/other"),
        visible(process(200, 100, 20, APP)),
    ];
    assert_eq!(app_root(&chain, Images::Exact), Some(1));
}

#[test]
fn reused_parent_pids_and_unknown_births_are_not_followed() {
    // The real parent exited; a younger process now has its PID.
    let younger = [
        process(300, 200, 30, APP),
        visible(process(200, 100, 35, APP)),
    ];
    assert_eq!(app_root(&younger, Images::Exact), None);
    let unknown = [
        process(300, 200, 30, APP),
        visible(process(200, 100, 0, APP)),
    ];
    assert_eq!(app_root(&unknown, Images::Exact), None);
    // The chain must really link child to parent.
    let unlinked = [
        process(300, 999, 30, APP),
        visible(process(200, 100, 20, APP)),
    ];
    assert_eq!(app_root(&unlinked, Images::Exact), None);
}

#[test]
fn other_users_sessions_services_and_critical_processes_stop_the_walk() {
    let helper = process(300, 200, 30, APP);
    let parent = visible(process(200, 100, 20, APP));
    for changed in [
        Lineage {
            owner: Some("root".into()),
            ..parent.clone()
        },
        Lineage {
            owner: None,
            ..parent.clone()
        },
        Lineage {
            session: Some(2),
            ..parent.clone()
        },
        Lineage {
            service: true,
            ..parent.clone()
        },
        Lineage {
            critical: true,
            ..parent.clone()
        },
        Lineage {
            executable: PathBuf::new(),
            ..parent.clone()
        },
    ] {
        assert_eq!(
            app_root(&[helper.clone(), changed.clone()], Images::Exact),
            None,
            "{changed:?}"
        );
    }
    assert_eq!(app_root(&[helper, parent], Images::Exact), Some(1));
}

#[test]
fn the_walk_is_capped() {
    let mut chain: Vec<_> = (0..12u32)
        .map(|i| process(1000 - i, 999 - i, 100 - u64::from(i), APP))
        .collect();
    chain[11].visible = true;
    assert_eq!(app_root(&chain, Images::Exact), None);
    chain[MAX_DEPTH].visible = true;
    assert_eq!(app_root(&chain, Images::Exact), Some(MAX_DEPTH));
}

#[test]
fn windows_compares_executables_without_case() {
    let chain = [
        process(
            300,
            200,
            30,
            r"C:\Users\k\AppData\Local\Programs\Example\Example.exe",
        ),
        visible(process(
            200,
            100,
            20,
            r"c:\users\k\appdata\local\programs\example\EXAMPLE.EXE",
        )),
    ];
    assert_eq!(app_root(&chain, Images::IgnoreCase), Some(1));
    assert_eq!(app_root(&chain, Images::Exact), None);
}

#[test]
fn macos_helpers_inside_the_bundle_resolve_to_its_main_executable() {
    let main = "/Applications/Example.app/Contents/MacOS/Example";
    let helper = "/Applications/Example.app/Contents/Frameworks/Example Helper (Renderer).app/Contents/MacOS/Example Helper (Renderer)";
    assert!(bundle_main_executable(Path::new(main)));
    assert!(!bundle_main_executable(Path::new(helper)));
    let chain = [
        process(300, 200, 30, helper),
        visible(process(200, 1, 20, main)),
    ];
    assert_eq!(app_root(&chain, Images::Bundle), Some(1));
    // Another bundle, such as a launcher app, is a different app.
    let other = [
        process(300, 200, 30, helper),
        visible(process(
            200,
            1,
            20,
            "/Applications/Launcher.app/Contents/MacOS/Launcher",
        )),
    ];
    assert_eq!(app_root(&other, Images::Bundle), None);
}

#[test]
fn desktop_app_scopes_are_read_from_cgroup_v2() {
    assert_eq!(
        desktop_app_scope(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-example-4242.scope\n"
        ),
        Some("app-gnome-example-4242.scope".into())
    );
    assert_eq!(
        desktop_app_scope(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/pipewire.service\n"
        ),
        None
    );
    assert_eq!(desktop_app_scope("0::/\n"), None);
}

fn entry(pid: u32, parent: u32, started: u64) -> ProcessEntry {
    ProcessEntry {
        identity: Identity {
            pid,
            started,
            started_sub: 0,
        },
        parent,
        name: format!("p{pid}"),
    }
}

#[test]
fn descendants_cover_every_level_and_skip_reused_pids() {
    let root = Identity {
        pid: 200,
        started: 20,
        started_sub: 0,
    };
    let table = [
        entry(200, 100, 20),
        entry(310, 200, 31),
        entry(400, 310, 40),
        entry(320, 200, 32),
        // Claims 200 as parent but is older: an earlier PID 200 started it.
        entry(150, 200, 15),
        entry(500, 999, 50),
    ];
    assert_eq!(
        descendants_of(&table, root)
            .iter()
            .map(|identity| identity.pid)
            .collect::<Vec<_>>(),
        [310, 320, 400]
    );
}
