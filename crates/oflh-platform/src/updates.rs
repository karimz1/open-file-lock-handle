//! Bounded, read-only access to the public stable-release manifest.
use oflh_core::releases::{ReleaseVersionError, newer_stable_release};
use reqwest::{blocking::Client, redirect::Policy};
use std::{io::Read, time::Duration};
use thiserror::Error;

/// The same stable manifest used by the desktop updater.
pub const MANIFEST_URL: &str = "https://oflh.karimzouine.com/api/latest.json";
/// Compatibility endpoint retained for existing installations and website outages.
pub const LEGACY_MANIFEST_URL: &str =
    "https://github.com/karimz1/open-file-lock-handle/releases/latest/download/latest.json";
/// Manual installation information; never a remotely supplied link.
pub const RELEASE_PAGE: &str = "https://github.com/karimz1/open-file-lock-handle/releases/latest";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// A release-check failure with operation context and typed original sources.
#[derive(Debug, Error)]
pub enum UpdateCheckError {
    /// Client setup, TLS, redirects, HTTP status or network failure.
    #[error("release request: {0}")]
    Request(#[source] reqwest::Error),
    /// Reading the bounded response failed, preserving OS codes.
    #[error("read release manifest: {0}")]
    Read(#[source] std::io::Error),
    /// The response exceeded the supported manifest size.
    #[error("release manifest exceeds 64 KiB")]
    TooLarge,
    /// JSON was malformed.
    #[error("parse release manifest: {0}")]
    Json(#[source] serde_json::Error),
    /// The stable manifest lacked a string version.
    #[error("release manifest has no string version")]
    MissingVersion,
    /// Version syntax was invalid.
    #[error(transparent)]
    Version(#[from] ReleaseVersionError),
}

fn request_error(error: reqwest::Error) -> UpdateCheckError {
    // GitHub asset redirects may carry signed queries. Keep the typed failure,
    // but never propagate a URL containing those temporary credentials to UI/logs.
    UpdateCheckError::Request(error.without_url())
}
fn allowed_redirect(url: &reqwest::Url, previous: usize) -> bool {
    previous < 5
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "oflh.karimzouine.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
fn redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        if allowed_redirect(attempt.url(), attempt.previous().len()) {
            attempt.follow()
        } else {
            attempt.error("unsupported release redirect")
        }
    })
}
fn client() -> Result<Client, UpdateCheckError> {
    Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(3))
        .https_only(true)
        // This public check never uses proxy credentials from the environment.
        .no_proxy()
        .redirect(redirect_policy())
        .user_agent("oflh-update-check")
        .build()
        .map_err(request_error)
}
fn read_manifest(
    mut reader: impl Read,
    installed: &str,
) -> Result<Option<String>, UpdateCheckError> {
    let mut bytes = Vec::with_capacity(4096);
    reader
        .by_ref()
        .take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(UpdateCheckError::Read)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(UpdateCheckError::TooLarge);
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(UpdateCheckError::Json)?;
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or(UpdateCheckError::MissingVersion)?;
    Ok(newer_stable_release(installed, version)?)
}
fn fetch(client: &Client, url: &str, installed: &str) -> Result<Option<String>, UpdateCheckError> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(request_error)?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MANIFEST_BYTES as u64)
    {
        return Err(UpdateCheckError::TooLarge);
    }
    read_manifest(response, installed)
}
/// Check for a newer stable release. Call only from a dedicated background worker.
/// Requests contain no inspection data. TLS verification remains enabled.
/// Each endpoint request is limited to eight seconds and 64 KiB; a failed primary
/// check may make one additional bounded fallback request. No binary is downloaded.
pub fn check_stable_release(installed: &str) -> Result<Option<String>, UpdateCheckError> {
    fetch_with_fallback(&client()?, MANIFEST_URL, LEGACY_MANIFEST_URL, installed)
}

fn fetch_with_fallback(
    client: &Client,
    primary: &str,
    fallback: &str,
    installed: &str,
) -> Result<Option<String>, UpdateCheckError> {
    // A valid response, including "no update", is authoritative. Retry only a
    // failed request or invalid manifest; each request retains its own bounds.
    fetch(client, primary, installed).or_else(|_| fetch(client, fallback, installed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, sync::mpsc, thread};
    #[test]
    fn manifest_validation_bounds_data_and_keeps_original_read_errors() {
        assert_eq!(
            read_manifest(
                &br#"{"version":"1.2.0","platforms":{},"notes":"ignored"}"#[..],
                "1.1.0"
            )
            .unwrap(),
            Some("1.2.0".into())
        );
        for bytes in [&b"{}"[..], &b"{\"version\":12}"[..]] {
            assert!(matches!(
                read_manifest(bytes, "1.0.0"),
                Err(UpdateCheckError::MissingVersion)
            ));
        }
        assert!(matches!(
            read_manifest(&b"{"[..], "1.0.0"),
            Err(UpdateCheckError::Json(_))
        ));
        assert!(matches!(
            read_manifest(&vec![b' '; MAX_MANIFEST_BYTES + 1][..], "1.0.0"),
            Err(UpdateCheckError::TooLarge)
        ));
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from_raw_os_error(5))
            }
        }
        assert!(
            matches!(read_manifest(Broken,"1.0.0"), Err(UpdateCheckError::Read(error)) if error.raw_os_error()==Some(5))
        );
    }
    #[test]
    fn redirect_policy_rejects_credentials_downgrade_other_hosts_and_loops() {
        for url in [
            "http://github.com/release",
            "https://github.com.evil.invalid/",
            "https://user:secret@github.com/",
            "https://github.com:444/",
            "https://example.invalid/",
        ] {
            assert!(!allowed_redirect(&url.parse().unwrap(), 1));
        }
        assert!(allowed_redirect(&MANIFEST_URL.parse().unwrap(), 1));
        assert!(allowed_redirect(&LEGACY_MANIFEST_URL.parse().unwrap(), 1));
        assert!(allowed_redirect(
            &"https://release-assets.githubusercontent.com/asset?token=synthetic"
                .parse()
                .unwrap(),
            4
        ));
        assert!(!allowed_redirect(&MANIFEST_URL.parse().unwrap(), 5));
        assert!(client().unwrap().get("http://127.0.0.1:1/").send().is_err());
    }
    #[test]
    fn fallback_recovers_http_and_invalid_metadata_without_leaking_urls() {
        let test_client = Client::builder().no_proxy().build().unwrap();
        for (status, body) in [
            (200, "{"),
            (200, "{}"),
            (200, "{\"version\":\"invalid\"}"),
            (503, ""),
        ] {
            let response = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let (primary, primary_thread, _primary_requests, _) =
                local_response(response.into_bytes(), false);
            let (fallback, fallback_thread, _fallback_requests, _) = local_response(
                b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"version\":\"1.2.0\"}".to_vec(),
                false,
            );
            assert_eq!(
                fetch_with_fallback(&test_client, &primary, &fallback, "1.0.0").unwrap(),
                Some("1.2.0".into())
            );
            primary_thread.join().unwrap();
            fallback_thread.join().unwrap();
        }
        let (primary, primary_thread, _primary_requests, _) = local_response(
            b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            false,
        );
        let (fallback, fallback_thread, _fallback_requests, _) = local_response(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            false,
        );
        let error = fetch_with_fallback(&test_client, &primary, &fallback, "1.0.0").unwrap_err();
        assert!(!error.to_string().contains("synthetic-secret"));
        assert!(matches!(error, UpdateCheckError::Request(_)));
        primary_thread.join().unwrap();
        fallback_thread.join().unwrap();
    }

    #[test]
    fn valid_primary_does_not_contact_fallback() {
        let test_client = Client::builder().no_proxy().build().unwrap();
        for installed in ["1.0.0", "1.2.0", "2.0.0"] {
            let (primary, worker, _requests, _) = local_response(
                b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"version\":\"1.2.0\"}".to_vec(),
                false,
            );
            let result =
                fetch_with_fallback(&test_client, &primary, "invalid URL", installed).unwrap();
            assert_eq!(result.is_some(), installed == "1.0.0");
            worker.join().unwrap();
        }
    }
    fn local_response(
        response: Vec<u8>,
        stalled: bool,
    ) -> (
        String,
        thread::JoinHandle<()>,
        mpsc::Receiver<String>,
        mpsc::Sender<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_tx, request_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                if socket.read(&mut byte).unwrap() == 0 {
                    break;
                }
                request.push(byte[0]);
            }
            request_tx
                .send(String::from_utf8(request).unwrap())
                .unwrap();
            if stalled {
                if !response.is_empty() {
                    let _ = socket.write_all(&response);
                }
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            } else {
                let _ = socket.write_all(&response);
            }
        });
        (
            format!("http://{address}/manifest?token=synthetic-secret"),
            thread,
            request_rx,
            release_tx,
        )
    }
    #[test]
    fn offline_http_handles_status_size_timeout_and_sends_only_public_headers() {
        let test_client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(1))
            .redirect(redirect_policy())
            .user_agent("oflh-update-check")
            .build()
            .unwrap();
        for (response,stalled,expected) in [
            (b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n{\"version\":\"1.2.0\"}".to_vec(),false,"update"),
            (b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),false,"status"),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n".to_vec(),false,"size"),
            (b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/asset?token=synthetic-secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),false,"redirect"),
            (Vec::new(),true,"timeout"),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\nConnection: close\r\n\r\n".to_vec(),true,"body-timeout"),
        ] {
            let (url,thread,requests,release)=local_response(response,stalled);
            let result=fetch(&test_client,&url,"1.0.0");
            match expected {
                "update"=>assert_eq!(result.as_ref().unwrap(),&Some("1.2.0".into())),
                "status"=>assert!(matches!(&result,Err(UpdateCheckError::Request(error)) if error.status()==Some(reqwest::StatusCode::SERVICE_UNAVAILABLE))),
                "size"=>assert!(matches!(&result,Err(UpdateCheckError::TooLarge))),
                "redirect"=>assert!(matches!(&result,Err(UpdateCheckError::Request(error)) if error.is_redirect())),
                "timeout"=>assert!(matches!(&result,Err(UpdateCheckError::Request(error)) if error.is_timeout())),
                _=>assert!(matches!(&result,Err(UpdateCheckError::Read(_)))),
            }
            if let Err(error)=result {assert!(!error.to_string().contains("synthetic-secret"));}
            let request=requests.recv_timeout(Duration::from_secs(2)).unwrap().to_lowercase();
            assert!(request.contains("user-agent: oflh-update-check"));
            assert!(request.contains("accept: application/json"));
            assert!(!request.contains("authorization:"));
            assert!(!request.contains("cookie:"));
            let _=release.send(());
            thread.join().unwrap();
        }
    }
}
