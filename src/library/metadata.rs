use super::{artwork, Track};
use anyhow::{Context, Result};
use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use std::path::Path;

pub fn read(path: &Path, cache_dir: &Path, external_art: Option<&Path>) -> Result<Track> {
    let tagged = match Probe::open(path)
        .context("Не удалось открыть аудиофайл")?
        .read()
    {
        Ok(tagged) => tagged,
        Err(error) => {
            // A damaged tag should not hide audio that Symphonia can still decode.
            let mut decoder =
                crate::audio::decoder::AudioDecoder::open(path).with_context(|| {
                    format!("Не удалось прочитать метаданные ({error}) и звуковую дорожку")
                })?;
            if decoder.decode_next()?.is_none() {
                anyhow::bail!("В файле нет звуковой дорожки");
            }
            let folder = path.parent().unwrap_or_else(|| Path::new("."));
            return Ok(Track {
                id: 0,
                path: path.to_path_buf(),
                title: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                artist: "Неизвестный исполнитель".into(),
                album: folder
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                album_key: folder.to_string_lossy().into_owned(),
                number: 0,
                duration: decoder.duration,
                artwork: external_art.map(Path::to_path_buf),
                fingerprint: String::new(),
            });
        }
    };
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let folder = path.parent().unwrap_or_else(|| Path::new("."));
    let fallback_album = folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let title = tag
        .and_then(|tag| tag.title())
        .map(|value| value.into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let artist = tag
        .and_then(|tag| tag.artist())
        .map(|value| value.into_owned())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "Неизвестный исполнитель".into());
    let album = tag
        .and_then(|tag| tag.album())
        .map(|value| value.into_owned())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(fallback_album);
    let artwork = tag
        .into_iter()
        .chain(tagged.tags().iter())
        .find_map(|tag| {
            let picture = tag
                .pictures()
                .iter()
                .find(|picture| picture.pic_type() == lofty::picture::PictureType::CoverFront)
                .or_else(|| tag.pictures().first())?;
            artwork::cache(picture.data(), path, cache_dir)
        })
        .or_else(|| external_art.map(Path::to_path_buf));
    let number = tag.and_then(|tag| tag.track()).unwrap_or_else(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    });
    Ok(Track {
        id: 0,
        path: path.to_path_buf(),
        title,
        artist,
        album,
        album_key: folder.to_string_lossy().into_owned(),
        number,
        duration: tagged.properties().duration().as_secs_f64(),
        artwork,
        fingerprint: String::new(),
    })
}
