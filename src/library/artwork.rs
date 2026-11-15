use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub fn external(folder: &Path) -> Option<PathBuf> {
    let mut images: Vec<PathBuf> = fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    matches!(
                        ext.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "webp"
                    )
                })
        })
        .collect();
    images.sort_by_key(|path| {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        (
            if matches!(name.as_str(), "cover" | "folder" | "front") {
                0
            } else {
                1
            },
            name,
        )
    });
    images.into_iter().find(|path| {
        image::ImageReader::open(path)
            .ok()
            .and_then(|r| r.with_guessed_format().ok())
            .and_then(|r| r.decode().ok())
            .is_some()
    })
}

pub fn cache(bytes: &[u8], source: &Path, cache_dir: &Path) -> Option<PathBuf> {
    if bytes.len() > 20 * 1024 * 1024 {
        return None;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hash);
    bytes.hash(&mut hash);
    let path = cache_dir.join(format!("{:016x}.jpg", hash.finish()));
    let small = thumbnail_path(&path);
    if path.exists() && small.exists() {
        return Some(path);
    }
    let artwork = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(600, 600)
        .to_rgb8();
    fs::create_dir_all(cache_dir).ok()?;
    artwork.save(&path).ok()?;
    image::imageops::thumbnail(&artwork, 80, 80)
        .save(&small)
        .ok()?;
    Some(path)
}

pub fn thumbnail_path(path: &Path) -> PathBuf {
    path.with_extension("small.jpg")
}

pub fn cache_external(path: &Path, cache_dir: &Path) -> Option<PathBuf> {
    if fs::metadata(path).ok()?.len() > 20 * 1024 * 1024 {
        return None;
    }
    cache(&fs::read(path).ok()?, path, cache_dir)
}
