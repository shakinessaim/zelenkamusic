//! Exercises the real scanner and both audio codecs without requiring an output device.
use anyhow::{ensure, Context, Result};
use std::{collections::HashSet, path::PathBuf};
use zelenka_music::{
    audio::decoder::{AudioDecoder, Resampler},
    library::scanner,
};

fn main() -> Result<()> {
    let songs = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("songs"));
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("verification");
    let library = scanner::scan(&songs, &output.join("artwork"), &output.join("library.db"))?;
    println!(
        "Discovered {} tracks across {} albums; {} warnings",
        library.tracks.len(),
        library.albums.len(),
        library.warnings.len()
    );
    ensure!(
        !library.tracks.is_empty(),
        "No MP3 or FLAC tracks were discovered in {}",
        songs.display()
    );
    let mut codecs = HashSet::new();
    for track in &library.tracks {
        let extension = track
            .path
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        if !codecs.insert(extension.clone()) {
            continue;
        }
        let mut decoder =
            AudioDecoder::open(&track.path).with_context(|| track.path.display().to_string())?;
        let chunk = decoder
            .decode_next()?
            .context("Audio contained no samples")?;
        let mut resampler = Resampler::new();
        let samples = resampler.convert(chunk, 48_000);
        ensure!(
            !samples.is_empty() && samples.iter().flatten().all(|sample| sample.is_finite()),
            "Invalid decoded audio"
        );
        if track.duration > 5.0 {
            decoder.seek(track.duration / 2.0)?;
            ensure!(decoder.decode_next()?.is_some(), "No samples after seeking");
        }
        println!(
            "{extension}: decode, 48 kHz resampling, seek OK — {}",
            track.title
        );
    }
    for warning in &library.warnings {
        eprintln!("{warning}");
    }
    println!("Verification finished. Codecs exercised: {codecs:?}");
    Ok(())
}
