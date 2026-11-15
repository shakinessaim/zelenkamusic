pub mod artwork;
pub mod metadata;
pub mod scanner;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_key: String,
    pub number: u32,
    pub duration: f64,
    pub artwork: Option<PathBuf>,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Clone, Debug)]
pub struct Album {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub artwork: Option<PathBuf>,
    pub tracks: Vec<i64>,
}

#[derive(Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub warnings: Vec<String>,
}

impl Library {
    pub fn track(&self, id: i64) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == id)
    }
    pub fn ids(&self) -> Vec<i64> {
        self.tracks.iter().map(|track| track.id).collect()
    }
}
