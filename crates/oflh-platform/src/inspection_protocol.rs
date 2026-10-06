//! Bounded binary transport for private native observations; never log its payloads.
use oflh_core::{AccessKind, Error, Identity, Result, io};
use std::io::{Read, Write};

const MAGIC: &[u8] = b"OFLH-HANDLES-1\0";
const MAX_FRAME: usize = 128 * 1024;
const MAX_PATH: usize = 32_768;

#[derive(Debug, PartialEq)]
pub(crate) struct Request {
    pub path: Vec<u16>,
    pub owner: Identity,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Observation {
    pub identity: Identity,
    pub path: Vec<u16>,
    pub directory: bool,
    pub mapped: bool,
    pub deleted: bool,
    pub sharing: Option<AccessKind>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Progress {
    pub processes: u64,
    pub handles: u64,
    pub names: u64,
    pub regions: u64,
    pub mapped_names: u64,
    pub snapshots: u64,
    pub snapshot_micros: u64,
    pub workers: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub(crate) enum Failure {
    Process = 1,
    Handle = 2,
    Path = 3,
    Mapping = 4,
    Alias = 5,
    Worker = 6,
    DeletedName = 7,
}
impl Failure {
    #[cfg(windows)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Process => "open handle source process",
            Self::Handle => "duplicate file handle",
            Self::Path => "resolve file handle path",
            Self::Mapping => "inspect data mapping",
            Self::Alias => "inspect file aliases",
            Self::Worker => "run native inspection worker",
            Self::DeletedName => "resolve original folder of deleted handle",
        }
    }
}
#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    Hello,
    Request(Request),
    Observation(Observation),
    Progress(Progress),
    Warning {
        operation: Failure,
        code: Option<i32>,
        count: u64,
    },
    Done,
    Failed {
        code: Option<i32>,
        reason: String,
    },
}

pub(crate) fn write_magic(writer: &mut impl Write) -> Result<()> {
    writer
        .write_all(MAGIC)
        .map_err(|error| io("write inspection helper greeting", error))
}
pub(crate) fn read_magic(reader: &mut impl Read) -> Result<()> {
    // The integration-test executable can emit a short harness preamble. Bound
    // that allowance so an unrelated executable cannot produce unbounded output.
    let mut matched = 0;
    for _ in 0..4096 {
        let mut byte = [0u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|error| io("read inspection helper greeting", error))?;
        matched = if byte[0] == MAGIC[matched] {
            matched + 1
        } else {
            usize::from(byte[0] == MAGIC[0])
        };
        if matched == MAGIC.len() {
            return Ok(());
        }
    }
    Err(Error::Unavailable(
        "inspection helper greeting is missing".into(),
    ))
}

fn append_identity(bytes: &mut Vec<u8>, identity: Identity) {
    bytes.extend(identity.pid.to_le_bytes());
    bytes.extend(identity.started.to_le_bytes());
    bytes.extend(identity.started_sub.to_le_bytes());
}
fn append_path(bytes: &mut Vec<u8>, path: &[u16]) -> Result<()> {
    if path.is_empty() || path.len() > MAX_PATH || path.contains(&0) {
        return Err(Error::Unavailable(
            "invalid inspection helper path extent".into(),
        ));
    }
    bytes.extend((path.len() as u32).to_le_bytes());
    for character in path {
        bytes.extend(character.to_le_bytes());
    }
    Ok(())
}
pub(crate) fn write_message(writer: &mut impl Write, message: &Message) -> Result<()> {
    let mut bytes = Vec::new();
    match message {
        Message::Hello => bytes.push(0),
        Message::Request(request) => {
            bytes.push(1);
            append_identity(&mut bytes, request.owner);
            append_path(&mut bytes, &request.path)?;
        }
        Message::Observation(observation) => {
            bytes.push(2);
            append_identity(&mut bytes, observation.identity);
            bytes.push(
                u8::from(observation.directory)
                    | (u8::from(observation.mapped) << 1)
                    | (u8::from(observation.deleted) << 2),
            );
            bytes.push(match observation.sharing {
                None => 0,
                Some(AccessKind::Read) => 1,
                Some(AccessKind::Write) => 2,
                Some(AccessKind::Delete) => 3,
            });
            append_path(&mut bytes, &observation.path)?;
        }
        Message::Progress(progress) => {
            bytes.push(3);
            for value in [
                progress.processes,
                progress.handles,
                progress.names,
                progress.regions,
                progress.mapped_names,
                progress.snapshots,
                progress.snapshot_micros,
                progress.workers,
            ] {
                bytes.extend(value.to_le_bytes());
            }
        }
        Message::Warning {
            operation,
            code,
            count,
        } => {
            bytes.extend([4, *operation as u8, u8::from(code.is_some())]);
            bytes.extend(code.unwrap_or(0).to_le_bytes());
            bytes.extend(count.to_le_bytes());
        }
        Message::Done => bytes.push(5),
        Message::Failed { code, reason } => {
            if reason.len() > 2048 {
                return Err(Error::Unavailable(
                    "helper failure detail exceeds extent".into(),
                ));
            }
            bytes.extend([6, u8::from(code.is_some())]);
            bytes.extend(code.unwrap_or(0).to_le_bytes());
            bytes.extend((reason.len() as u32).to_le_bytes());
            bytes.extend(reason.as_bytes());
        }
    }
    if bytes.len() > MAX_FRAME {
        return Err(Error::Unavailable(
            "inspection helper frame exceeds budget".into(),
        ));
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|()| writer.write_all(&bytes))
        .map_err(|error| io("write inspection helper frame", error))
}

struct Decoder<'a>(&'a [u8]);
impl Decoder<'_> {
    fn take<const SIZE: usize>(&mut self) -> Result<[u8; SIZE]> {
        let (field, rest) = self
            .0
            .split_at_checked(SIZE)
            .ok_or_else(|| Error::Unavailable("truncated inspection helper frame".into()))?;
        self.0 = rest;
        field
            .try_into()
            .map_err(|_| Error::Unavailable("invalid inspection helper field".into()))
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    fn number(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn identity(&mut self) -> Result<Identity> {
        let identity = Identity {
            pid: u32::from_le_bytes(self.take()?),
            started: self.number()?,
            started_sub: self.number()?,
        };
        if identity.pid == 0 || identity.started == 0 || identity.started_sub != 0 {
            return Err(Error::Unavailable(
                "invalid inspection helper process identity".into(),
            ));
        }
        Ok(identity)
    }
    fn path(&mut self) -> Result<Vec<u16>> {
        let count = u32::from_le_bytes(self.take()?) as usize;
        if count == 0 || count > MAX_PATH || count > self.0.len() / 2 {
            return Err(Error::Unavailable(
                "invalid inspection helper path length".into(),
            ));
        }
        let path = (0..count)
            .map(|_| self.take().map(u16::from_le_bytes))
            .collect::<Result<Vec<_>>>()?;
        if path.contains(&0) {
            return Err(Error::Unavailable(
                "inspection helper path contains NUL".into(),
            ));
        }
        Ok(path)
    }
}
fn decode(bytes: &[u8]) -> Result<Message> {
    let mut decoder = Decoder(bytes);
    let message = match decoder.byte()? {
        0 => Message::Hello,
        1 => Message::Request(Request {
            owner: decoder.identity()?,
            path: decoder.path()?,
        }),
        2 => {
            let identity = decoder.identity()?;
            let flags = decoder.byte()?;
            if flags & !7 != 0 {
                return Err(Error::Unavailable(
                    "invalid inspection helper observation flags".into(),
                ));
            }
            let sharing = match decoder.byte()? {
                0 => None,
                1 => Some(AccessKind::Read),
                2 => Some(AccessKind::Write),
                3 => Some(AccessKind::Delete),
                _ => return Err(Error::Unavailable("invalid helper sharing evidence".into())),
            };
            Message::Observation(Observation {
                identity,
                path: decoder.path()?,
                directory: flags & 1 != 0,
                mapped: flags & 2 != 0,
                deleted: flags & 4 != 0,
                sharing,
            })
        }
        3 => Message::Progress(Progress {
            processes: decoder.number()?,
            handles: decoder.number()?,
            names: decoder.number()?,
            regions: decoder.number()?,
            mapped_names: decoder.number()?,
            snapshots: decoder.number()?,
            snapshot_micros: decoder.number()?,
            workers: decoder.number()?,
        }),
        4 => {
            let operation = match decoder.byte()? {
                1 => Failure::Process,
                2 => Failure::Handle,
                3 => Failure::Path,
                4 => Failure::Mapping,
                5 => Failure::Alias,
                6 => Failure::Worker,
                7 => Failure::DeletedName,
                _ => {
                    return Err(Error::Unavailable(
                        "unknown inspection helper operation".into(),
                    ));
                }
            };
            let has_code = decoder.byte()?;
            if has_code > 1 {
                return Err(Error::Unavailable(
                    "invalid inspection helper error flag".into(),
                ));
            }
            let code = i32::from_le_bytes(decoder.take()?);
            let count = decoder.number()?;
            if count == 0 {
                return Err(Error::Unavailable(
                    "invalid inspection helper failure count".into(),
                ));
            }
            Message::Warning {
                operation,
                code: (has_code == 1).then_some(code),
                count,
            }
        }
        5 => Message::Done,
        6 => {
            let has_code = decoder.byte()?;
            let code = i32::from_le_bytes(decoder.take()?);
            let length = u32::from_le_bytes(decoder.take()?) as usize;
            if has_code > 1 || length > 2048 || length > decoder.0.len() {
                return Err(Error::Unavailable(
                    "invalid helper failure detail extent".into(),
                ));
            }
            let (detail, rest) = decoder.0.split_at(length);
            let reason = std::str::from_utf8(detail)
                .map_err(|_| Error::Unavailable("invalid helper failure text".into()))?
                .to_owned();
            decoder.0 = rest;
            Message::Failed {
                code: (has_code == 1).then_some(code),
                reason,
            }
        }
        _ => {
            return Err(Error::Unavailable(
                "unknown inspection helper message".into(),
            ));
        }
    };
    if !decoder.0.is_empty() {
        return Err(Error::Unavailable(
            "inspection helper frame has unexpected fields".into(),
        ));
    }
    Ok(message)
}
pub(crate) fn read_message(reader: &mut impl Read) -> Result<Message> {
    let mut length = [0u8; 4];
    reader
        .read_exact(&mut length)
        .map_err(|error| io("read inspection helper frame length", error))?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(Error::Unavailable(
            "invalid inspection helper frame length".into(),
        ));
    }
    let mut bytes = vec![0u8; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| io("read inspection helper frame", error))?;
    decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_paths_and_births_round_trip_without_loss() {
        let identity = Identity {
            pid: 123,
            started: 456,
            started_sub: 0,
        };
        let path = vec![b'C' as u16, b':' as u16, b'\\' as u16, 0xd800, b'x' as u16];
        for message in [
            Message::Hello,
            Message::Request(Request {
                path: path.clone(),
                owner: identity,
            }),
            Message::Observation(Observation {
                identity,
                path,
                directory: false,
                mapped: true,
                deleted: true,
                sharing: Some(AccessKind::Write),
            }),
            Message::Progress(Progress {
                names: 1_000_000,
                ..Progress::default()
            }),
            Message::Warning {
                operation: Failure::Mapping,
                code: Some(-1),
                count: 160,
            },
            Message::Done,
            Message::Failed {
                code: Some(5),
                reason: "open source process".into(),
            },
        ] {
            let mut encoded = Vec::new();
            write_message(&mut encoded, &message).unwrap();
            assert_eq!(read_message(&mut encoded.as_slice()).unwrap(), message);
        }
    }
    #[test]
    fn malformed_frames_are_rejected_without_truncating_results() {
        for bytes in [
            vec![],
            vec![99],
            vec![5, 0],
            vec![4, 255, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        ] {
            assert!(decode(&bytes).is_err());
        }
        let identity = Identity {
            pid: 123,
            started: 456,
            started_sub: 0,
        };
        let mut bytes = Vec::new();
        write_message(
            &mut bytes,
            &Message::Observation(Observation {
                identity,
                path: vec![65],
                directory: false,
                mapped: false,
                deleted: false,
                sharing: None,
            }),
        )
        .unwrap();
        for length in 4..bytes.len() {
            assert!(read_message(&mut &bytes[..length]).is_err());
        }
        assert!(read_message(&mut (MAX_FRAME as u32 + 1).to_le_bytes().as_slice()).is_err());
        assert!(
            write_message(
                &mut Vec::new(),
                &Message::Request(Request {
                    owner: identity,
                    path: vec![65; MAX_PATH + 1]
                })
            )
            .is_err()
        );
        assert!(
            write_message(
                &mut Vec::new(),
                &Message::Request(Request {
                    owner: identity,
                    path: vec![0]
                })
            )
            .is_err()
        );
    }
    #[test]
    fn greeting_resynchronization_is_bounded() {
        let mut bytes = b"test harness preamble\n".to_vec();
        write_magic(&mut bytes).unwrap();
        read_magic(&mut bytes.as_slice()).unwrap();
        assert!(read_magic(&mut vec![65; 5000].as_slice()).is_err());
    }
}
