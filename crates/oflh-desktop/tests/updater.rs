//! Real updater HTTP and signature coverage; no installer is launched by these tests.
#![cfg(feature = "updater-tests")]

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri_plugin_updater::UpdaterExt;

const PAYLOAD: &[u8] = include_bytes!("../../../xtask/tests/fixtures/updater/payload");
const SIGNATURE: &str = include_str!("../../../xtask/tests/fixtures/updater/payload.sig");
const PUBLIC_KEY: &str = include_str!("../../../xtask/tests/fixtures/updater/public.key");

struct UpdateServer {
    endpoint: url::Url,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl UpdateServer {
    fn start(version: &str, payload: Vec<u8>, status: u16) -> Self {
        Self::start_signed(version, payload, status, SIGNATURE)
    }

    fn start_signed(version: &str, payload: Vec<u8>, status: u16, signature: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let manifest = serde_json::json!({
            "version": version,
            "notes": "Synthetic RC for updater regression coverage",
            "platforms": {
                "test-platform": {
                    "url": format!("http://{address}/payload"),
                    "signature": signature
                }
            }
        })
        .to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !worker_stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((stream, _)) => serve_request(stream, &manifest, &payload, status),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("updater fixture server failed: {error}"),
                }
            }
        });
        Self {
            endpoint: format!("http://{address}/latest.json").parse().unwrap(),
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for UpdateServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Preserve the original test failure instead of aborting on a second panic.
        if let Some(worker) = self.worker.take()
            && let Err(failure) = worker.join()
            && !thread::panicking()
        {
            std::panic::resume_unwind(failure);
        }
    }
}

fn serve_request(mut stream: TcpStream, manifest: &str, payload: &[u8], status: u16) {
    // Accepted sockets inherit nonblocking mode on macOS. Large package responses
    // must wait for the updater to drain the socket rather than fail with WouldBlock.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0; 1024];
    while request.len() < 8192 && !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
    }
    let is_payload = request.starts_with(b"GET /payload ");
    let body = if is_payload {
        payload
    } else {
        manifest.as_bytes()
    };
    let status = if is_payload { 200 } else { status };
    write!(
        stream,
        "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
}

fn build_updater(
    server: &UpdateServer,
    public_key: &str,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri_plugin_updater::Updater,
) {
    build_updater_with_version(server, public_key, "1.0.0-rc.1")
}

fn build_updater_with_version(
    server: &UpdateServer,
    public_key: &str,
    current_version: &str,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri_plugin_updater::Updater,
) {
    let mut context = mock_context(noop_assets());
    context.package_info_mut().version = current_version.parse().unwrap();
    // HTTP is permitted only in this loopback test configuration, never in the app config.
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": "",
            "dangerousInsecureTransportProtocol": true
        }),
    );
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    // Tauri stores the entire minisign public-key text as base64.
    let updater = app
        .updater_builder()
        .pubkey(public_key)
        .target("test-platform")
        .endpoints(vec![server.endpoint.clone()])
        .unwrap()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .unwrap();
    (app, updater)
}

#[test]
fn failed_primary_endpoint_falls_back_to_a_verified_download() {
    let primary = UpdateServer::start("9.9.9", PAYLOAD.to_vec(), 503);
    let fallback = UpdateServer::start("9.9.9", PAYLOAD.to_vec(), 200);
    let (app, _) = build_updater(&primary, PUBLIC_KEY.trim());
    let updater = app
        .updater_builder()
        .pubkey(PUBLIC_KEY.trim())
        .target("test-platform")
        .endpoints(vec![primary.endpoint.clone(), fallback.endpoint.clone()])
        .unwrap()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .unwrap();
    tauri::async_runtime::block_on(async {
        let update = updater.check().await.unwrap().unwrap();
        assert_eq!(update.version, "9.9.9");
        assert_eq!(update.download(|_, _| {}, || {}).await.unwrap(), PAYLOAD);
    });
}

#[test]
fn newer_rc_is_detected_and_signed_payload_downloads() {
    let server = UpdateServer::start("9.9.9-rc.1", PAYLOAD.to_vec(), 200);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    tauri::async_runtime::block_on(async {
        let update = updater.check().await.unwrap().unwrap();
        assert_eq!(update.version, "9.9.9-rc.1");
        let mut downloaded = 0;
        let mut finished = false;
        let payload = update
            .download(|count, _| downloaded += count, || finished = true)
            .await
            .unwrap();
        assert_eq!(payload, PAYLOAD);
        assert_eq!(downloaded, PAYLOAD.len());
        assert!(finished);
    });
}

#[test]
fn current_and_older_versions_do_not_offer_an_update() {
    for version in ["1.0.0-rc.1", "1.0.0-beta.1", "0.9.0"] {
        let server = UpdateServer::start(version, PAYLOAD.to_vec(), 200);
        let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
        assert!(
            tauri::async_runtime::block_on(updater.check())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn tampered_payload_is_rejected_before_installation() {
    let server = UpdateServer::start("9.9.9-rc.1", b"tampered payload".to_vec(), 200);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    tauri::async_runtime::block_on(async {
        let update = updater.check().await.unwrap().unwrap();
        assert!(matches!(
            update.download(|_, _| {}, || {}).await,
            Err(tauri_plugin_updater::Error::Minisign(_))
        ));
    });
}

#[test]
fn endpoint_failure_is_reported_and_no_content_means_no_update() {
    let server = UpdateServer::start("9.9.9-rc.1", PAYLOAD.to_vec(), 500);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    assert!(tauri::async_runtime::block_on(updater.check()).is_err());
    let server = UpdateServer::start("9.9.9-rc.1", PAYLOAD.to_vec(), 204);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    assert!(
        tauri::async_runtime::block_on(updater.check())
            .unwrap()
            .is_none()
    );
}

#[test]
fn malformed_release_version_is_rejected() {
    let server = UpdateServer::start("not-semver", PAYLOAD.to_vec(), 200);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    assert!(tauri::async_runtime::block_on(updater.check()).is_err());
}

#[test]
#[ignore = "requires a native CI bundle and its signature via OFLH_UPDATER_PACKAGE"]
fn actual_packaged_update_is_detected_and_downloaded() {
    let package_path =
        std::env::var("OFLH_UPDATER_PACKAGE").expect("CI must supply the tested package");
    let payload = std::fs::read(&package_path).unwrap();
    let signature = std::fs::read_to_string(format!("{package_path}.sig")).unwrap();
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let public_key = config
        .pointer("/plugins/updater/pubkey")
        .unwrap()
        .as_str()
        .unwrap();
    let package_version =
        std::env::var("OFLH_VERSION").expect("CI must supply the signed build version");
    let server =
        UpdateServer::start_signed(&package_version, payload.clone(), 200, signature.trim());
    let (_app, updater) = build_updater_with_version(&server, public_key, "0.0.0");
    tauri::async_runtime::block_on(async {
        let update = updater.check().await.unwrap().unwrap();
        assert_eq!(update.version, package_version);
        assert_eq!(update.download(|_, _| {}, || {}).await.unwrap(), payload);
    });
}

#[test]
fn announced_version_must_match_the_signed_artifact_version() {
    let server = UpdateServer::start("9.9.9-rc.2", PAYLOAD.to_vec(), 200);
    let (_app, updater) = build_updater(&server, PUBLIC_KEY.trim());
    tauri::async_runtime::block_on(async {
        let update = updater.check().await.unwrap().unwrap();
        assert!(matches!(
            update.download(|_, _| {}, || {}).await,
            Err(tauri_plugin_updater::Error::SignedVersionMismatch { .. })
        ));
    });
}

#[test]
fn large_response_completes_when_the_accepted_socket_was_nonblocking() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let payload = vec![b'x'; 8 * 1024 * 1024];
    let expected_length = payload.len();
    let worker = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(true).unwrap();
        serve_request(stream, "{}", &payload, 200);
    });
    let mut client = TcpStream::connect(address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(b"GET /payload HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    // Leave enough backpressure to exercise a response larger than the send buffer.
    thread::sleep(Duration::from_millis(100));
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    worker.join().unwrap();
    let header_end = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert_eq!(response.len() - header_end, expected_length);
    assert!(response[header_end..].iter().all(|byte| *byte == b'x'));
}
