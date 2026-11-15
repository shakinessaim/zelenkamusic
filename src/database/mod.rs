use crate::library::Track;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub struct Database {
    connection: Connection,
    pub path: PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(3))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS tracks (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, artist TEXT NOT NULL, album TEXT NOT NULL, album_key TEXT NOT NULL, track_number INTEGER NOT NULL, duration REAL NOT NULL, artwork TEXT, available INTEGER NOT NULL DEFAULT 1, fingerprint TEXT NOT NULL DEFAULT '');
            CREATE TABLE IF NOT EXISTS playlists (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS playlist_tracks (playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE, track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE, position INTEGER NOT NULL, PRIMARY KEY(playlist_id, track_id));")?;
        let has_fingerprint: bool = connection.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name='fingerprint'",
            [],
            |row| row.get::<_, i64>(0).map(|count| count > 0),
        )?;
        if !has_fingerprint {
            connection.execute(
                "ALTER TABLE tracks ADD COLUMN fingerprint TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row("SELECT value FROM preferences WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn sync_tracks(&mut self, tracks: &mut [Track]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("UPDATE tracks SET available=0", [])?;
        for track in tracks {
            transaction.execute("INSERT INTO tracks(path,title,artist,album,album_key,track_number,duration,artwork,available,fingerprint) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,?9) ON CONFLICT(path) DO UPDATE SET title=excluded.title,artist=excluded.artist,album=excluded.album,album_key=excluded.album_key,track_number=excluded.track_number,duration=excluded.duration,artwork=excluded.artwork,available=1,fingerprint=excluded.fingerprint", params![track.path.to_string_lossy(), track.title, track.artist, track.album, track.album_key, track.number, track.duration, track.artwork.as_ref().map(|path| path.to_string_lossy().into_owned()), track.fingerprint])?;
            track.id = transaction.query_row(
                "SELECT id FROM tracks WHERE path=?1",
                [track.path.to_string_lossy()],
                |row| row.get(0),
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn cached_track(&self, path: &Path, fingerprint: &str) -> Result<Option<Track>> {
        let track = self.connection.query_row("SELECT id,title,artist,album,album_key,track_number,duration,artwork FROM tracks WHERE path=?1 AND fingerprint=?2", params![path.to_string_lossy(), fingerprint], |row| {
            Ok(Track { id: row.get(0)?, path: path.to_path_buf(), title: row.get(1)?, artist: row.get(2)?, album: row.get(3)?, album_key: row.get(4)?, number: row.get(5)?, duration: row.get(6)?, artwork: row.get::<_, Option<String>>(7)?.map(PathBuf::from), fingerprint: fingerprint.to_owned() })
        }).optional()?;
        Ok(track.filter(|track| {
            track.artwork.as_ref().is_none_or(|path| {
                path.is_file() && crate::library::artwork::thumbnail_path(path).is_file()
            })
        }))
    }
}
