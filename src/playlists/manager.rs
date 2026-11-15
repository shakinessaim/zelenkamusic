use crate::database::Database;
use anyhow::{bail, Result};
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub tracks: Vec<i64>,
}

pub fn list(database: &Database) -> Result<Vec<Playlist>> {
    let connection = database.connection();
    let mut statement = connection.prepare("SELECT id,name FROM playlists ORDER BY id DESC")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut playlists = Vec::new();
    for row in rows {
        let (id, name) = row?;
        let mut tracks = connection.prepare("SELECT pt.track_id FROM playlist_tracks pt JOIN tracks t ON t.id=pt.track_id WHERE pt.playlist_id=?1 AND t.available=1 ORDER BY pt.position")?;
        let ids = tracks
            .query_map([id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        playlists.push(Playlist {
            id,
            name,
            tracks: ids,
        });
    }
    Ok(playlists)
}

fn validate(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Введите название плейлиста.");
    }
    if name.chars().count() > 80 {
        bail!("Название плейлиста не должно превышать 80 символов.");
    }
    Ok(name)
}

pub fn create(database: &Database, name: &str) -> Result<i64> {
    database
        .connection()
        .execute("INSERT INTO playlists(name) VALUES(?1)", [validate(name)?])?;
    Ok(database.connection().last_insert_rowid())
}
pub fn rename(database: &Database, id: i64, name: &str) -> Result<()> {
    database.connection().execute(
        "UPDATE playlists SET name=?1 WHERE id=?2",
        params![validate(name)?, id],
    )?;
    Ok(())
}
pub fn delete(database: &Database, id: i64) -> Result<()> {
    database
        .connection()
        .execute("DELETE FROM playlists WHERE id=?1", [id])?;
    Ok(())
}
pub fn add(database: &Database, playlist: i64, track: i64) -> Result<()> {
    database.connection().execute("INSERT OR IGNORE INTO playlist_tracks(playlist_id,track_id,position) SELECT ?1,?2,COALESCE(MAX(position),-1)+1 FROM playlist_tracks WHERE playlist_id=?1", params![playlist, track])?;
    Ok(())
}
pub fn remove(database: &Database, playlist: i64, track: i64) -> Result<()> {
    database.connection().execute(
        "DELETE FROM playlist_tracks WHERE playlist_id=?1 AND track_id=?2",
        params![playlist, track],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        library::Track,
        settings::{Repeat, Settings},
    };

    #[test]
    fn playlists_and_preferences_survive_reopen_and_missing_music() -> Result<()> {
        let file = std::env::temp_dir().join(format!(
            "zelenka-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut database = Database::open(&file)?;
        let mut tracks = vec![Track {
            id: 0,
            path: "C:\\songs\\album\\track.flac".into(),
            title: "Track".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            album_key: "album".into(),
            number: 1,
            duration: 123.0,
            artwork: None,
            fingerprint: String::new(),
        }];
        database.sync_tracks(&mut tracks)?;
        let playlist = create(&database, "  Evening  ")?;
        add(&database, playlist, tracks[0].id)?;
        add(&database, playlist, tracks[0].id)?; // Duplicate additions are idempotent.
        let settings = Settings {
            snow: true,
            volume: 0.42,
            shuffle: false,
            repeat: Repeat::One,
        };
        settings.save(&database)?;
        drop(database);
        let mut database = Database::open(&file)?;
        assert_eq!(list(&database)?[0].name, "Evening");
        assert_eq!(list(&database)?[0].tracks, vec![tracks[0].id]);
        let restored = Settings::load(&database);
        assert!(restored.snow);
        assert_eq!(restored.volume, 0.42);
        assert_eq!(restored.repeat, Repeat::One);
        database.sync_tracks(&mut [])?;
        assert!(list(&database)?[0].tracks.is_empty());
        database.sync_tracks(&mut tracks)?;
        assert_eq!(list(&database)?[0].tracks, vec![tracks[0].id]);
        rename(&database, playlist, "New name")?;
        assert_eq!(list(&database)?[0].name, "New name");
        remove(&database, playlist, tracks[0].id)?;
        assert!(list(&database)?[0].tracks.is_empty());
        delete(&database, playlist)?;
        assert!(list(&database)?.is_empty());
        drop(database);
        std::fs::remove_file(file)?;
        Ok(())
    }
}
