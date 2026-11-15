use super::{artwork, metadata, Album, Library};
use crate::database::Database;
use anyhow::Result;
use std::{collections::BTreeMap, fs, path::Path};

pub fn scan(songs: &Path, cache: &Path, database: &Path) -> Result<Library> {
    let mut library = Library::default();
    let mut database = Database::open(database)?;
    if !songs.is_dir() {
        library.warnings.push(format!(
            "Поместите альбомы с MP3 или FLAC в {}",
            songs.display()
        ));
        database.sync_tracks(&mut library.tracks)?;
        return Ok(library);
    }
    let mut folders = vec![songs.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => {
                library
                    .warnings
                    .push(format!("{}: {error}", folder.display()));
                continue;
            }
        };
        let external =
            artwork::external(&folder).and_then(|path| artwork::cache_external(&path, cache));
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    library.warnings.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            // Do not follow directory junctions or symlinks outside the music library.
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(_) => continue,
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                folders.push(path);
                continue;
            }
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp3" | "flac"))
            {
                continue;
            }
            let stat = match entry.metadata() {
                Ok(stat) => stat,
                Err(error) => {
                    library
                        .warnings
                        .push(format!("{}: {error}", path.display()));
                    continue;
                }
            };
            let modified = stat
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_nanos())
                .unwrap_or(0);
            let fingerprint = format!(
                "v1:{}:{modified}:{}",
                stat.len(),
                external
                    .as_ref()
                    .map(|path| path.to_string_lossy())
                    .unwrap_or_default()
            );
            if let Some(track) = database.cached_track(&path, &fingerprint)? {
                library.tracks.push(track);
                continue;
            }
            match metadata::read(&path, cache, external.as_deref()) {
                Ok(mut track) => {
                    track.fingerprint = fingerprint;
                    library.tracks.push(track);
                }
                Err(error) => library
                    .warnings
                    .push(format!("Пропущен файл {}: {error:#}", path.display())),
            }
        }
    }
    library.tracks.sort_by(|a, b| {
        (&a.album_key, a.number, &a.title).cmp(&(&b.album_key, b.number, &b.title))
    });
    database.sync_tracks(&mut library.tracks)?;
    let mut groups: BTreeMap<String, Vec<&super::Track>> = BTreeMap::new();
    for track in &library.tracks {
        groups
            .entry(track.album_key.clone())
            .or_default()
            .push(track);
    }
    for (key, tracks) in groups {
        let first = tracks[0];
        let artist = if tracks.iter().all(|track| track.artist == first.artist) {
            first.artist.clone()
        } else {
            "Разные исполнители".into()
        };
        library.albums.push(Album {
            key,
            title: first.album.clone(),
            artist,
            artwork: tracks.iter().find_map(|track| track.artwork.clone()),
            tracks: tracks.iter().map(|track| track.id).collect(),
        });
    }
    Ok(library)
}
