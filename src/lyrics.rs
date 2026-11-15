//! Manually supplied UTF-8/UTF-16 text and LRC lyrics, independent of playback.
use anyhow::{bail, Context, Result};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Default, Debug)]
pub struct Lyrics {
    pub text: String,
    cues: Vec<(f64, String)>,
}

impl Lyrics {
    pub fn parse(source: &str) -> Self {
        let mut result = Self::default();
        let mut text = Vec::new();
        let mut offset = 0.0;
        for line in source.trim_start_matches('\u{feff}').lines() {
            let mut remaining = line.trim_end();
            let mut timestamps = Vec::new();
            while let Some(end) = remaining
                .strip_prefix('[')
                .and_then(|s| s.find(']').map(|i| i + 1))
            {
                let tag = &remaining[1..end];
                if let Some(value) = timestamp(tag) {
                    timestamps.push(value);
                    remaining = &remaining[end + 1..];
                } else if timestamps.is_empty() && tag.starts_with("offset:") {
                    offset = tag[7..]
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0)
                        / 1000.0;
                    remaining = "";
                    break;
                } else if timestamps.is_empty()
                    && ["ar:", "ti:", "al:", "by:", "re:", "ve:", "length:"]
                        .iter()
                        .any(|prefix| tag.starts_with(prefix))
                {
                    remaining = "";
                    break;
                } else {
                    break;
                }
            }
            if timestamps.is_empty() {
                if !remaining.is_empty() || line.trim().is_empty() {
                    text.push(remaining.to_owned());
                }
            } else {
                let lyric = remaining.trim().to_owned();
                text.push(lyric.clone());
                result
                    .cues
                    .extend(timestamps.into_iter().map(|time| (time, lyric.clone())));
            }
        }
        for (time, _) in &mut result.cues {
            *time = (*time - offset).max(0.0);
        }
        result.cues.sort_by(|a, b| a.0.total_cmp(&b.0));
        result.text = text.join("\n").trim().to_owned();
        result
    }

    pub fn synchronized(&self) -> bool {
        !self.cues.is_empty()
    }

    pub fn current_line(&self, seconds: f64) -> String {
        if !seconds.is_finite() {
            return String::new();
        }
        let end = self.cues.partition_point(|(time, _)| *time <= seconds);
        if end == 0 {
            return String::new();
        }
        let time = self.cues[end - 1].0;
        let start = self.cues[..end].partition_point(|(cue, _)| *cue < time);
        self.cues[start..end]
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn timestamp(tag: &str) -> Option<f64> {
    let (minutes, seconds) = tag.split_once(':')?;
    if minutes.is_empty() || !minutes.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if seconds.is_empty()
        || !seconds.bytes().all(|c| c.is_ascii_digit() || c == b'.')
        || seconds.split('.').any(str::is_empty)
    {
        return None;
    }
    let seconds = seconds.parse::<f64>().ok()?;
    if !seconds.is_finite() || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    let time = minutes.parse::<u32>().ok()? as f64 * 60.0 + seconds;
    Some(time)
}

pub fn expected_path(root: &Path, songs: &Path, track: &Path) -> PathBuf {
    root.join(
        track.strip_prefix(songs).unwrap_or(
            track
                .file_name()
                .map(Path::new)
                .unwrap_or(Path::new("track")),
        ),
    )
    .with_extension("txt")
}

pub fn load(root: &Path, songs: &Path, track: &Path) -> Result<Option<(PathBuf, Lyrics)>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let root_canonical = root.canonicalize()?;
    let mirrored = expected_path(root, songs, track);
    let flat = root.join(
        track
            .file_name()
            .context("Композиция не имеет имени файла")?,
    );
    for base in [mirrored, flat] {
        for extension in ["lrc", "txt"] {
            let path = base.with_extension(extension);
            if !path.is_file() {
                continue;
            }
            if !path.canonicalize()?.starts_with(&root_canonical) {
                bail!("Файл текста находится вне папки lyrics");
            }
            let mut bytes = Vec::new();
            File::open(&path)?
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_BYTES {
                bail!("Файл текста больше 1 МБ: {}", path.display());
            }
            let source = decode(&bytes)
                .with_context(|| format!("Сохраните текст как UTF-8: {}", path.display()))?;
            return Ok(Some((path, Lyrics::parse(&source))));
        }
    }
    Ok(None)
}

pub fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if !bytes.len().is_multiple_of(2) {
            bail!("Повреждённый UTF-16");
        }
        let little = bytes[0] == 0xff;
        let words = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect::<Vec<_>>();
        return Ok(String::from_utf16(&words)?);
    }
    Ok(std::str::from_utf8(bytes)?
        .trim_start_matches('\u{feff}')
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_and_encodings() {
        assert_eq!(
            Lyrics::parse("\u{feff}Первая\r\n\r\nВторая").text,
            "Первая\n\nВторая"
        );
        assert_eq!(decode(&[0xff, 0xfe, 0x1f, 0x04]).unwrap(), "П");
        assert!(decode(&[0xff, 0xfe, 0]).is_err());
    }
    #[test]
    fn lrc_timing_offset_duplicates_and_seek() {
        let lyrics = Lyrics::parse("[ar:Artist]\n[00:10.50][00:30.500]Chorus\n[00:20]Verse\n[00:20]Harmony\n[00:25]\n[offset:500]");
        assert!(lyrics.synchronized());
        assert_eq!(lyrics.current_line(9.9), "");
        assert_eq!(lyrics.current_line(10.0), "Chorus");
        assert_eq!(lyrics.current_line(19.5), "Verse\nHarmony");
        assert_eq!(lyrics.current_line(24.5), "");
        assert_eq!(lyrics.current_line(30.0), "Chorus");
        assert_eq!(lyrics.current_line(11.0), "Chorus");
        assert_eq!(
            Lyrics::parse("[00:99]Invalid\n[00:NaN]Literal").text,
            "[00:99]Invalid\n[00:NaN]Literal"
        );
    }
    #[test]
    fn mirrored_files_take_precedence_and_edits_reload() -> Result<()> {
        let root = std::env::temp_dir().join(format!(
            "zelenka-lyrics-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let lyrics = root.join("lyrics");
        let songs = root.join("songs");
        std::fs::create_dir_all(lyrics.join("Album"))?;
        let track = songs.join("Album/01 - Song.flac");
        let mirrored = lyrics.join("Album/01 - Song.txt");
        std::fs::write(lyrics.join("01 - Song.lrc"), "[00:00]Flat")?;
        std::fs::write(&mirrored, "First")?;
        assert_eq!(load(&lyrics, &songs, &track)?.unwrap().1.text, "First");
        std::fs::write(&mirrored, "Edited")?;
        assert_eq!(load(&lyrics, &songs, &track)?.unwrap().1.text, "Edited");
        assert!(load(&lyrics, &songs, &songs.join("Missing.mp3"))?.is_none());
        std::fs::remove_file(mirrored)?;
        std::fs::remove_file(lyrics.join("01 - Song.lrc"))?;
        std::fs::remove_dir(lyrics.join("Album"))?;
        std::fs::remove_dir(lyrics)?;
        std::fs::remove_dir(root)?;
        Ok(())
    }
}
