use crate::database::Database;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Repeat {
    Off,
    #[default]
    All,
    One,
}
impl Repeat {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
    pub fn code(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::All => 1,
            Self::One => 2,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub snow: bool,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            snow: false,
            volume: 0.75,
            shuffle: true,
            repeat: Repeat::All,
        }
    }
}
impl Settings {
    pub fn load(database: &Database) -> Self {
        let mut settings: Self = database
            .get("settings")
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        if !settings.volume.is_finite() {
            settings.volume = 0.75;
        }
        settings.volume = settings.volume.clamp(0.0, 1.0);
        settings
    }
    pub fn save(&self, database: &Database) -> anyhow::Result<()> {
        database.set("settings", &serde_json::to_string(self)?)
    }
}
