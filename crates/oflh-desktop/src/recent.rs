use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

pub(crate) struct RecentTargets {
    connection: Connection,
}

impl RecentTargets {
    pub(crate) fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    pub(crate) fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS recent_targets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path BLOB NOT NULL UNIQUE
            );",
        )?;
        Ok(Self { connection })
    }

    pub(crate) fn load(&self) -> rusqlite::Result<Vec<PathBuf>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM recent_targets ORDER BY id DESC LIMIT 12")?;
        let rows = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
        rows.map(|row| decode_path(row?)).collect()
    }

    pub(crate) fn record(&mut self, path: &Path) -> rusqlite::Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM recent_targets WHERE path = ?1",
            params![encode_path(path)],
        )?;
        transaction.execute(
            "INSERT INTO recent_targets (path) VALUES (?1)",
            params![encode_path(path)],
        )?;
        transaction.execute(
            "DELETE FROM recent_targets WHERE id NOT IN (
                SELECT id FROM recent_targets ORDER BY id DESC LIMIT 12
            )",
            [],
        )?;
        transaction.commit()
    }

    pub(crate) fn remove(&mut self, path: &Path) -> rusqlite::Result<()> {
        self.connection.execute(
            "DELETE FROM recent_targets WHERE path = ?1",
            params![encode_path(path)],
        )?;
        Ok(())
    }

    pub(crate) fn clear(&mut self) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM recent_targets", [])?;
        Ok(())
    }
}

#[cfg(unix)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn encode_path(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn decode_path(bytes: Vec<u8>) -> rusqlite::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(windows)]
fn decode_path(bytes: Vec<u8>) -> rusqlite::Result<PathBuf> {
    use std::io;
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Blob,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid native path encoding",
            )),
        ));
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

#[cfg(not(any(unix, windows)))]
fn decode_path(bytes: Vec<u8>) -> rusqlite::Result<PathBuf> {
    use std::io;
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Blob,
                Box::new(io::Error::new(io::ErrorKind::InvalidData, error)),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

    struct TestDatabase(PathBuf);

    impl TestDatabase {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "oflh-recent-{}-{nonce}-{sequence}.sqlite3",
                std::process::id()
            )))
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn recent_targets_survive_reopening_and_keep_native_paths() {
        let database = TestDatabase::new();
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(
                b"/tmp/oflh-\xff-target".to_vec(),
            ))
        };
        #[cfg(not(unix))]
        let path = PathBuf::from("/tmp/oflh-recent-target");

        {
            let mut recent = RecentTargets::open(&database.0).unwrap();
            recent.record(&path).unwrap();
        }

        let recent = RecentTargets::open(&database.0).unwrap();
        assert_eq!(recent.load().unwrap(), vec![path]);
    }

    #[test]
    fn recent_targets_keep_the_newest_twelve_and_move_revisited_paths_forward() {
        let database = TestDatabase::new();
        let mut recent = RecentTargets::open(&database.0).unwrap();
        let paths = (0..13)
            .map(|index| PathBuf::from(format!("/tmp/oflh-target-{index}")))
            .collect::<Vec<_>>();

        for path in &paths {
            recent.record(path).unwrap();
        }
        recent.record(&paths[4]).unwrap();

        let loaded = recent.load().unwrap();
        assert_eq!(loaded.len(), 12);
        assert_eq!(loaded[0], paths[4]);
        assert_eq!(loaded[1], paths[12]);
        assert!(!loaded.contains(&paths[0]));
    }

    #[test]
    fn recent_targets_can_be_removed_individually_or_cleared() {
        let database = TestDatabase::new();
        let first = PathBuf::from("/tmp/oflh-first");
        let second = PathBuf::from("/tmp/oflh-second");
        let mut recent = RecentTargets::open(&database.0).unwrap();
        recent.record(&first).unwrap();
        recent.record(&second).unwrap();

        recent.remove(&first).unwrap();
        assert_eq!(recent.load().unwrap(), vec![second]);
        recent.clear().unwrap();
        assert!(recent.load().unwrap().is_empty());
    }
}
