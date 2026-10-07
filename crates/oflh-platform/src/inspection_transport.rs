//! Owned helper lifetime and bounded pipe queues. Native queries never run here.
use crate::{inspection_helper::InspectionHelperCommand, inspection_protocol::*};
use oflh_core::{Cancellation, Error, Result, io};
use std::{
    io::{BufReader, BufWriter, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(crate) enum Outcome {
    Complete,
    /// No valid greeting: the embedding binary may not implement the helper.
    Unavailable(Error),
    /// Keep completed observations, but explicitly disclose incomplete work.
    Partial(Error),
}

struct OwnedHelper {
    child: Child,
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    #[cfg(windows)]
    _job: super::windows::helper_job::Job,
}
impl Drop for OwnedHelper {
    fn drop(&mut self) {
        // Only our helper is killed. Closing its pipe endpoints releases both
        // bounded I/O threads; the receiver must be dropped before this guard.
        let _ = self.child.kill();
        let _ = self.child.wait();
        for handle in [&mut self.reader, &mut self.writer] {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
    }
}

fn start(
    configuration: &InspectionHelperCommand,
    request: Request,
) -> Result<(OwnedHelper, Receiver<Result<Message>>)> {
    let mut command = Command::new(configuration.executable());
    command
        .args(configuration.arguments())
        .env("OFLH_NATIVE_HANDLE_HELPER", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|error| io("start native inspection helper", error))?;
    #[cfg(windows)]
    let job = match super::windows::helper_job::Job::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut helper = OwnedHelper {
        child,
        reader: None,
        writer: None,
        #[cfg(windows)]
        _job: job,
    };
    let stdout = helper
        .child
        .stdout
        .take()
        .ok_or_else(|| Error::Unavailable("inspection helper output pipe is missing".into()))?;
    let stdin = helper
        .child
        .stdin
        .take()
        .ok_or_else(|| Error::Unavailable("inspection helper input pipe is missing".into()))?;
    let (sender, receiver) = mpsc::sync_channel(16);
    let writer_errors = sender.clone();
    helper.writer = Some(
        std::thread::Builder::new()
            .name("oflh-helper-input".into())
            .spawn(move || {
                let mut writer = BufWriter::new(stdin);
                let result =
                    write_message(&mut writer, &Message::Request(request)).and_then(|()| {
                        writer
                            .flush()
                            .map_err(|error| io("flush helper request", error))
                    });
                if let Err(error) = result {
                    let _ = writer_errors.send(Err(error));
                }
            })
            .map_err(|error| io("start inspection request writer", error))?,
    );
    helper.reader = Some(
        std::thread::Builder::new()
            .name("oflh-helper-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                if let Err(error) = read_magic(&mut reader) {
                    let _ = sender.send(Err(error));
                    return;
                }
                loop {
                    let message = read_message(&mut reader);
                    let last =
                        matches!(message, Ok(Message::Done | Message::Failed { .. }) | Err(_));
                    if sender.send(message).is_err() || last {
                        break;
                    }
                }
            })
            .map_err(|error| io("start inspection result reader", error))?,
    );
    Ok((helper, receiver))
}

pub(crate) fn inspect(
    configuration: &InspectionHelperCommand,
    request: Request,
    cancel: &Cancellation,
    mut receive: impl FnMut(Message) -> Result<()>,
) -> Result<Outcome> {
    inspect_with_limits(
        configuration,
        request,
        cancel,
        &mut receive,
        Duration::from_secs(5),
        Duration::from_secs(20),
    )
}

fn inspect_with_limits(
    configuration: &InspectionHelperCommand,
    request: Request,
    cancel: &Cancellation,
    receive: &mut impl FnMut(Message) -> Result<()>,
    startup_limit: Duration,
    stall_limit: Duration,
) -> Result<Outcome> {
    cancel.check()?;
    let (mut helper, receiver) = match start(configuration, request) {
        Ok(started) => started,
        Err(error) => return Ok(Outcome::Unavailable(error)),
    };
    let result = consume(
        &mut helper,
        &receiver,
        cancel,
        receive,
        startup_limit,
        stall_limit,
    );
    // Drop the receiver before joining a possibly blocked bounded sender.
    drop(receiver);
    drop(helper);
    result
}

fn consume(
    helper: &mut OwnedHelper,
    receiver: &Receiver<Result<Message>>,
    cancel: &Cancellation,
    receive: &mut impl FnMut(Message) -> Result<()>,
    startup_limit: Duration,
    stall_limit: Duration,
) -> Result<Outcome> {
    let mut greeted = false;
    let mut watch = WorkWatch::new(Instant::now());
    let mut activity = 0;
    let failure = loop {
        cancel.check()?;
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(Message::Hello)) if !greeted => {
                greeted = true;
                watch.note_output(Instant::now());
            }
            Ok(Ok(Message::Failed { code, reason })) => {
                break code.map_or_else(
                    || Error::Unavailable(reason),
                    |code| {
                        io(
                            "native inspection helper",
                            std::io::Error::from_raw_os_error(code),
                        )
                    },
                );
            }
            Ok(Ok(Message::Done)) if greeted => {
                let exit_started = Instant::now();
                while exit_started.elapsed() < startup_limit {
                    cancel.check()?;
                    if let Some(status) = helper
                        .child
                        .try_wait()
                        .map_err(|error| io("wait for inspection helper", error))?
                    {
                        if status.success() {
                            return Ok(Outcome::Complete);
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                break Error::Unavailable(
                    "inspection helper did not exit successfully after completion".into(),
                );
            }
            Ok(Ok(Message::Activity(mask))) if greeted => {
                // Diagnostic heartbeats cannot conceal a blocked native call.
                activity = mask;
            }
            Ok(Ok(Message::Progress(progress))) if greeted => {
                if let Err(error) = watch.note_progress(progress, Instant::now()) {
                    break error;
                }
                receive(Message::Progress(progress))?;
            }
            Ok(Ok(message @ (Message::Observation(_) | Message::Warning { .. }))) if greeted => {
                watch.note_output(Instant::now());
                receive(message)?;
            }
            Ok(Ok(_)) => break Error::Unavailable("unexpected inspection helper message".into()),
            Ok(Err(error)) => break error,
            Err(RecvTimeoutError::Disconnected) => {
                break Error::Unavailable("inspection helper exited without completion".into());
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        let limit = if greeted { stall_limit } else { startup_limit };
        if watch.expired(Instant::now(), limit) {
            break Error::Unavailable(format!(
                "inspection helper stalled during {}; partial observations retained",
                crate::inspection_protocol::activity_label(activity)
            ));
        }
    };
    Ok(if greeted {
        Outcome::Partial(failure)
    } else {
        Outcome::Unavailable(failure)
    })
}

/// Watch actual work rather than heartbeat traffic or total scan time.
struct WorkWatch {
    last_work: Instant,
    previous: Progress,
}
impl WorkWatch {
    fn new(now: Instant) -> Self {
        Self {
            last_work: now,
            previous: Progress::default(),
        }
    }
    fn note_output(&mut self, now: Instant) {
        self.last_work = now;
    }
    fn note_progress(&mut self, current: Progress, now: Instant) -> Result<()> {
        if !monotonic(self.previous, current) {
            return Err(Error::Unavailable(
                "inspection helper counters moved backwards".into(),
            ));
        }
        if current != self.previous {
            self.last_work = now;
        }
        self.previous = current;
        Ok(())
    }
    fn expired(&self, now: Instant, limit: Duration) -> bool {
        now.saturating_duration_since(self.last_work) >= limit
    }
}

fn monotonic(previous: Progress, current: Progress) -> bool {
    previous.processes <= current.processes
        && previous.handles <= current.handles
        && previous.names <= current.names
        && previous.regions <= current.regions
        && previous.mapped_names <= current.mapped_names
        && previous.snapshots <= current.snapshots
        && previous.snapshot_micros <= current.snapshot_micros
        && previous.workers <= current.workers
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::Identity;
    use std::{ffi::OsString, path::PathBuf};

    fn configuration() -> InspectionHelperCommand {
        InspectionHelperCommand::new(
            std::env::current_exe().unwrap(),
            [
                "--exact",
                "inspection_transport::tests::helper_fixture",
                "--nocapture",
            ]
            .map(OsString::from)
            .to_vec(),
        )
    }
    fn request(mode: &str) -> Request {
        Request {
            path: mode.encode_utf16().collect(),
            owner: Identity {
                pid: 123,
                started: 456,
                started_sub: 0,
            },
        }
    }
    fn observation() -> Message {
        Message::Observation(Observation {
            identity: Identity {
                pid: std::process::id(),
                started: 456,
                started_sub: 0,
            },
            path: vec![65],
            directory: false,
            mapped: false,
            deleted: false,
            sharing: None,
        })
    }
    #[test]
    fn helper_fixture() {
        if std::env::var_os("OFLH_NATIVE_HANDLE_HELPER").is_none() {
            return;
        }
        let mut output = BufWriter::new(std::io::stdout().lock());
        write_magic(&mut output).unwrap();
        write_message(&mut output, &Message::Hello).unwrap();
        output.flush().unwrap();
        let Message::Request(request) = read_message(&mut std::io::stdin().lock()).unwrap() else {
            panic!("missing fixture request")
        };
        match String::from_utf16(&request.path).unwrap().as_str() {
            "complete" => {
                for _ in 0..2000 {
                    write_message(&mut output, &observation()).unwrap();
                }
            }
            "truncated" => {
                write_message(&mut output, &observation()).unwrap();
                output.write_all(&128u32.to_le_bytes()).unwrap();
                output.write_all(&[2, 0]).unwrap();
                output.flush().unwrap();
                drop(output);
                std::process::exit(0);
            }
            "backwards" => {
                write_message(
                    &mut output,
                    &Message::Progress(Progress {
                        names: 2,
                        ..Progress::default()
                    }),
                )
                .unwrap();
                write_message(
                    &mut output,
                    &Message::Progress(Progress {
                        names: 1,
                        ..Progress::default()
                    }),
                )
                .unwrap();
            }
            "failed" => {
                write_message(&mut output, &observation()).unwrap();
                write_message(
                    &mut output,
                    &Message::Failed {
                        code: Some(5),
                        reason: "snapshot native handles".into(),
                    },
                )
                .unwrap();
                output.flush().unwrap();
                drop(output);
                std::process::exit(1);
            }
            "stall" => loop {
                write_message(&mut output, &Message::Activity(1 << 6)).unwrap();
                write_message(&mut output, &Message::Progress(Progress::default())).unwrap();
                output.flush().unwrap();
                std::thread::sleep(Duration::from_millis(10));
            },
            "backpressure" => loop {
                write_message(&mut output, &observation()).unwrap();
                output.flush().unwrap();
            },
            "slow" => {
                for names in 1..=8 {
                    std::thread::sleep(Duration::from_millis(200));
                    write_message(
                        &mut output,
                        &Message::Progress(Progress {
                            names,
                            ..Progress::default()
                        }),
                    )
                    .unwrap();
                    output.flush().unwrap();
                }
            }
            _ => panic!("unknown fixture mode"),
        }
        write_message(&mut output, &Message::Done).unwrap();
        output.flush().unwrap();
        drop(output);
        std::process::exit(0);
    }
    #[test]
    fn bounded_transport_retains_every_observation_and_requires_completion() {
        let cancel = Cancellation::default();
        let mut count = 0;
        let outcome = inspect(&configuration(), request("complete"), &cancel, |message| {
            if matches!(message, Message::Observation(_)) {
                count += 1;
            }
            Ok(())
        })
        .unwrap();
        assert!(matches!(outcome, Outcome::Complete), "{outcome:?}");
        assert_eq!(count, 2000);
        count = 0;
        let outcome = inspect(&configuration(), request("truncated"), &cancel, |message| {
            if matches!(message, Message::Observation(_)) {
                count += 1;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(count, 1);
        let Outcome::Partial(error) = outcome else {
            panic!("truncated output was accepted")
        };
        assert!(error.to_string().contains("read inspection helper frame"));
    }
    #[test]
    fn cancellation_reaps_helper_and_releases_bounded_pipe_backpressure() {
        let cancel = Cancellation::default();
        let started = Instant::now();
        let mut count = 0;
        let result = inspect(
            &configuration(),
            request("backpressure"),
            &cancel,
            |message| {
                if matches!(message, Message::Observation(_)) {
                    count += 1;
                    if count == 50 {
                        cancel.cancel();
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
                Ok(())
            },
        );
        assert!(matches!(result, Err(Error::Cancelled)));
        assert_eq!(count, 50);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[test]
    fn helper_failure_preserves_native_error_code_and_partial_observations() {
        let mut observed = 0;
        let outcome = inspect(
            &configuration(),
            request("failed"),
            &Cancellation::default(),
            |message| {
                if matches!(message, Message::Observation(_)) {
                    observed += 1;
                }
                Ok(())
            },
        )
        .unwrap();
        let Outcome::Partial(Error::Io { source, .. }) = outcome else {
            panic!("typed native error was lost")
        };
        assert_eq!(source.raw_os_error(), Some(5));
        assert_eq!(observed, 1);
    }
    #[test]
    fn heartbeats_cannot_hide_stalled_or_backwards_native_work() {
        for mode in ["stall", "backwards"] {
            let outcome = inspect_with_limits(
                &configuration(),
                request(mode),
                &Cancellation::default(),
                &mut |_| Ok(()),
                Duration::from_secs(5),
                Duration::from_millis(100),
            )
            .unwrap();
            let Outcome::Partial(error) = outcome else {
                panic!("invalid native progress was accepted")
            };
            if mode == "stall" {
                assert!(error.to_string().contains("probe file sharing"));
            }
            assert!(error.to_string().contains(if mode == "stall" {
                "stalled"
            } else {
                "backwards"
            }));
        }
        // Scale the real-process fixture above scheduler jitter. The exact
        // twenty-second deadline is independently tested with a controlled clock.
        let started = Instant::now();
        let outcome = inspect_with_limits(
            &configuration(),
            request("slow"),
            &Cancellation::default(),
            &mut |_| Ok(()),
            Duration::from_secs(5),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(matches!(outcome, Outcome::Complete), "{outcome:?}");
        assert!(started.elapsed() > Duration::from_secs(1));
    }
    #[test]
    fn missing_helper_is_explicitly_unavailable_and_precancelled_never_spawns() {
        let configuration = InspectionHelperCommand::new(
            PathBuf::from("missing-oflh-helper-executable"),
            Vec::new(),
        );
        let cancel = Cancellation::default();
        let Outcome::Unavailable(error) =
            inspect(&configuration, request("complete"), &cancel, |_| Ok(())).unwrap()
        else {
            panic!("missing helper accepted")
        };
        assert!(error.to_string().contains("start native inspection helper"));
        cancel.cancel();
        assert!(matches!(
            inspect(&configuration, request("complete"), &cancel, |_| Ok(())),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn work_deadline_uses_elapsed_since_real_progress_without_total_scan_cutoff() {
        let start = Instant::now();
        let limit = Duration::from_secs(20);
        let mut watch = WorkWatch::new(start);
        for names in 1..=100 {
            let now = start + Duration::from_secs(names * 19);
            watch
                .note_progress(
                    Progress {
                        names,
                        ..Progress::default()
                    },
                    now,
                )
                .unwrap();
            assert!(!watch.expired(now, limit));
        }
        let last = start + Duration::from_secs(1900);
        watch
            .note_progress(watch.previous, last + Duration::from_secs(19))
            .unwrap();
        assert!(watch.expired(last + limit, limit));
        assert!(
            watch
                .note_progress(Progress::default(), last + limit)
                .is_err()
        );
        watch.note_output(last + limit);
        assert!(!watch.expired(last + limit, limit));
    }
}
