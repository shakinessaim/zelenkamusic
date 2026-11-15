use crate::{
    audio::{
        playback_state::Queue,
        player::{Command, Event, Player},
    },
    database::Database,
    library::{scanner, Library},
    playlists::manager::{self, Playlist},
    settings::Settings,
    AppWindow, CardData, ChoiceData, TrackData,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Image, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

#[derive(Clone)]
enum Detail {
    Album(String),
    Playlist(i64),
}
#[derive(Serialize, Deserialize)]
struct Resume {
    track: i64,
    position: f64,
    tracks: Vec<i64>,
    home: bool,
}

struct App {
    window: slint::Weak<AppWindow>,
    database: Database,
    settings: Settings,
    player: Player,
    library: Library,
    playlists: Vec<Playlist>,
    queue: Queue,
    songs: PathBuf,
    cache: PathBuf,
    scan: Option<Receiver<Result<Library, String>>>,
    detail: Option<Detail>,
    detail_ids: Vec<i64>,
    images: HashMap<PathBuf, Image>,
    pending_add: Option<i64>,
    position: f64,
    duration: f64,
    playing: bool,
    token: u64,
    audio_available: bool,
    failed: HashSet<i64>,
    notice_until: Instant,
    last_save: Instant,
    dirty: bool,
    snow_start: Instant,
    first_scan: bool,
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(items)).into()
}
fn artist_name(artist: &str) -> String {
    match artist {
        "Unknown artist" => "Неизвестный исполнитель",
        "Various artists" => "Разные исполнители",
        other => other,
    }
    .to_owned()
}
fn quantity(count: usize, forms: [&str; 3]) -> String {
    let form = if (11..=14).contains(&(count % 100)) {
        2
    } else {
        match count % 10 {
            1 => 0,
            2..=4 => 1,
            _ => 2,
        }
    };
    format!("{count} {}", forms[form])
}
fn track_count(count: usize) -> String {
    quantity(count, ["композиция", "композиции", "композиций"])
}
fn time(seconds: f64) -> String {
    let seconds = if seconds.is_finite() {
        seconds.max(0.0) as u64
    } else {
        0
    };
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

impl App {
    fn ui(&self) -> Option<AppWindow> {
        self.window.upgrade()
    }
    fn notice(&mut self, message: impl Into<String>) {
        let message = message.into();
        if let Some(ui) = self.ui() {
            ui.set_notice(message.into());
        }
        self.notice_until = Instant::now() + Duration::from_secs(9);
    }
    fn report<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                if let Some(ui) = self.ui().filter(|ui| ui.get_modal() > 0) {
                    ui.set_modal_error(format!("{error:#}").into());
                } else {
                    self.notice(format!("{error:#}"));
                }
                None
            }
        }
    }
    fn image(&mut self, path: Option<PathBuf>) -> Image {
        let Some(path) = path else {
            return self.placeholder();
        };
        if let Some(image) = self.images.get(&path) {
            return image.clone();
        }
        let image = Image::load_from_path(&path).unwrap_or_else(|_| self.placeholder());
        // Bound retained large covers; track rows use separate 80-pixel thumbnails.
        let small = path.to_string_lossy().ends_with(".small.jpg");
        let limit = if small { 512 } else { 20 };
        let retained: Vec<_> = self
            .images
            .keys()
            .filter(|key| {
                key.as_os_str() != "__placeholder__"
                    && key.to_string_lossy().ends_with(".small.jpg") == small
            })
            .cloned()
            .collect();
        if retained.len() >= limit {
            if let Some(oldest) = retained.first() {
                self.images.remove(oldest);
            }
        }
        self.images.insert(path, image.clone());
        image
    }
    fn placeholder(&self) -> Image {
        // The compiled image lives in the window's initial artwork property.
        self.images
            .get(&PathBuf::from("__placeholder__"))
            .cloned()
            .unwrap_or_default()
    }
    fn settings_ui(&self) {
        if let Some(ui) = self.ui() {
            ui.set_snow(self.settings.snow);
            ui.set_volume(self.settings.volume);
            ui.set_shuffle(self.settings.shuffle);
            ui.set_repeat_mode(self.settings.repeat.code());
        }
    }
    fn save(&mut self) {
        let result = self.settings.save(&self.database);
        self.report(result);
        if let Some(track) = self.queue.current {
            let resume = Resume {
                track,
                position: self.position,
                tracks: self.queue.tracks.clone(),
                home: self.queue.home,
            };
            let result = serde_json::to_string(&resume)
                .map_err(anyhow::Error::from)
                .and_then(|value| self.database.set("resume", &value));
            self.report(result);
        }
        self.dirty = false;
        self.last_save = Instant::now();
    }
    fn start_scan(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let songs = self.songs.clone();
        let cache = self.cache.clone();
        let database = self.database.path.clone();
        match std::thread::Builder::new()
            .name("zelenka-library".into())
            .spawn(move || {
                let result =
                    scanner::scan(&songs, &cache, &database).map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.scan = Some(receiver);
                if let Some(ui) = self.ui() {
                    ui.set_scanning(true);
                }
            }
            Err(error) => {
                self.notice(format!("Не удалось начать поиск музыки: {error}"));
                if let Some(ui) = self.ui() {
                    ui.set_scanning(false);
                }
            }
        }
    }
    fn library_ui(&mut self) {
        let Some(ui) = self.ui() else {
            return;
        };
        let albums = self
            .library
            .albums
            .clone()
            .into_iter()
            .map(|album| CardData {
                title: album.title.into(),
                artist: artist_name(&album.artist).into(),
                subtitle: track_count(album.tracks.len()).into(),
                artwork: self.image(album.artwork),
            })
            .collect();
        ui.set_albums(model(albums));
        ui.set_has_library(!self.library.tracks.is_empty());
        ui.set_library_summary(if self.library.tracks.is_empty() {
            "Добавьте альбомы в папку songs.".into()
        } else {
            format!(
                "{} · {}",
                track_count(self.library.tracks.len()),
                quantity(self.library.albums.len(), ["альбом", "альбома", "альбомов"])
            )
            .into()
        });
        self.reload_playlists();
        self.detail_ui();
    }
    fn reload_playlists(&mut self) {
        let result = manager::list(&self.database);
        if let Some(playlists) = self.report(result) {
            self.playlists = playlists;
        }
        let cards = self
            .playlists
            .clone()
            .into_iter()
            .map(|playlist| {
                let art = playlist.tracks.iter().find_map(|id| {
                    self.library
                        .track(*id)
                        .and_then(|track| track.artwork.clone())
                });
                CardData {
                    title: playlist.name.into(),
                    artist: "Ваш плейлист".into(),
                    subtitle: track_count(playlist.tracks.len()).into(),
                    artwork: self.image(art),
                }
            })
            .collect();
        let choices = self
            .playlists
            .iter()
            .map(|playlist| ChoiceData {
                name: playlist.name.clone().into(),
                subtitle: track_count(playlist.tracks.len()).into(),
            })
            .collect();
        if let Some(ui) = self.ui() {
            ui.set_playlists(model(cards));
            ui.set_choices(model(choices));
        }
    }
    fn detail_ui(&mut self) {
        let Some(ui) = self.ui() else {
            return;
        };
        let data = match self.detail.clone() {
            Some(Detail::Album(key)) => self
                .library
                .albums
                .iter()
                .find(|album| album.key == key)
                .map(|album| {
                    (
                        album.title.clone(),
                        album.artist.clone(),
                        album.artwork.clone(),
                        album.tracks.clone(),
                        false,
                    )
                }),
            Some(Detail::Playlist(id)) => self
                .playlists
                .iter()
                .find(|playlist| playlist.id == id)
                .map(|playlist| {
                    let art = playlist.tracks.iter().find_map(|id| {
                        self.library
                            .track(*id)
                            .and_then(|track| track.artwork.clone())
                    });
                    (
                        playlist.name.clone(),
                        "Ваш плейлист".into(),
                        art,
                        playlist.tracks.clone(),
                        true,
                    )
                }),
            None => None,
        };
        let Some((title, artist, art, ids, playlist)) = data else {
            self.detail_ids.clear();
            ui.set_detail(false);
            return;
        };
        self.detail_ids = ids;
        ui.set_detail(true);
        ui.set_is_playlist(playlist);
        ui.set_detail_title(title.into());
        ui.set_detail_artist(artist_name(&artist).into());
        ui.set_detail_artwork(self.image(art));
        let total: f64 = self
            .detail_ids
            .iter()
            .filter_map(|id| self.library.track(*id))
            .map(|track| track.duration)
            .sum();
        ui.set_detail_summary(
            format!(
                "{} · {} мин",
                track_count(self.detail_ids.len()),
                (total / 60.0).round() as u64
            )
            .into(),
        );
        let tracks: Vec<_> = self
            .detail_ids
            .iter()
            .filter_map(|id| self.library.track(*id).cloned())
            .collect();
        ui.set_tracks(model(
            tracks
                .into_iter()
                .enumerate()
                .map(|(index, track)| TrackData {
                    title: track.title.into(),
                    artist: artist_name(&track.artist).into(),
                    duration: time(track.duration).into(),
                    number: (index + 1).to_string().into(),
                    artwork: self.image(
                        track
                            .artwork
                            .map(|path| crate::library::artwork::thumbnail_path(&path)),
                    ),
                    current: self.queue.current == Some(track.id),
                })
                .collect(),
        ));
    }
    fn open_album(&mut self, index: i32) {
        if let Some(album) = self.library.albums.get(index as usize) {
            self.detail = Some(Detail::Album(album.key.clone()));
            self.detail_ui();
        }
    }
    fn open_playlist(&mut self, index: i32) {
        if let Some(playlist) = self.playlists.get(index as usize) {
            self.detail = Some(Detail::Playlist(playlist.id));
            self.detail_ui();
        }
    }
    fn start(&mut self, tracks: Vec<i64>, chosen: Option<i64>, shuffle: bool, home: bool) {
        if tracks.is_empty() {
            self.notice("В этой подборке пока нет композиций.");
            return;
        }
        self.failed.clear();
        self.settings.shuffle = shuffle || home;
        if let Some(id) = self.queue.start(tracks, chosen, shuffle, home) {
            self.load(id, 0.0, true);
        }
        self.settings_ui();
        self.dirty = true;
    }
    fn load(&mut self, id: i64, position: f64, playing: bool) {
        if !self.audio_available {
            self.notice(
                "Аудиовыход недоступен. Подключите устройство и перезапустите ZelenkaMusic.",
            );
            return;
        }
        let Some(track) = self.library.track(id).cloned() else {
            self.notice("Композиция отсутствует в папке songs. Обновите медиатеку.");
            return;
        };
        self.token += 1;
        self.position = position;
        self.duration = track.duration;
        self.playing = playing;
        self.player.send(Command::Load {
            token: self.token,
            path: track.path,
            position,
            playing,
        });
        if let Some(ui) = self.ui() {
            ui.set_track_title(track.title.into());
            ui.set_track_artist(artist_name(&track.artist).into());
            ui.set_artwork(self.image(track.artwork));
            ui.set_has_track(true);
            ui.set_playing(playing);
        }
        self.progress_ui();
        self.detail_ui();
        self.dirty = true;
    }
    fn home_play(&mut self) {
        if self.queue.current.is_none() {
            self.start(self.library.ids(), None, true, true);
            return;
        }
        if !self.queue.home {
            let current = self.queue.current;
            self.queue.start(self.library.ids(), current, true, true);
        }
        self.queue.home = true;
        self.queue.set_shuffle(true);
        self.settings.shuffle = true;
        self.settings_ui();
        self.toggle();
        self.dirty = true;
    }
    fn toggle(&mut self) {
        if self.queue.current.is_none() {
            self.home_play();
            return;
        }
        if !self.audio_available {
            self.notice("Подключите аудиоустройство и перезапустите ZelenkaMusic.");
            return;
        }
        if !self.playing && self.duration > 0.0 && self.position >= self.duration - 0.1 {
            if let Some(id) = self.queue.current {
                self.load(id, 0.0, true);
            }
            return;
        }
        self.playing = !self.playing;
        self.player.send(Command::Playing(self.playing));
        if let Some(ui) = self.ui() {
            ui.set_playing(self.playing);
        }
        self.dirty = true;
    }
    fn next(&mut self, manual: bool) {
        if let Some(id) = self.queue.advance(self.settings.repeat, manual) {
            self.load(id, 0.0, true);
        } else {
            self.playing = false;
            if let Some(ui) = self.ui() {
                ui.set_playing(false);
            }
            self.dirty = true;
        }
    }
    fn previous(&mut self) {
        if self.position > 3.0 {
            self.seek(0.0);
        } else if let Some(id) = self.queue.previous() {
            self.load(id, 0.0, true);
        }
    }
    fn seek(&mut self, fraction: f32) {
        if self.queue.current.is_some() && self.duration > 0.0 {
            self.position = (fraction as f64).clamp(0.0, 1.0) * (self.duration - 0.05).max(0.0);
            self.token += 1;
            self.player.send(Command::Seek {
                token: self.token,
                position: self.position,
            });
            self.progress_ui();
            self.dirty = true;
        }
    }
    fn progress_ui(&self) {
        if let Some(ui) = self.ui() {
            ui.set_elapsed(time(self.position).into());
            ui.set_duration(time(self.duration).into());
            ui.set_progress(if self.duration > 0.0 {
                (self.position / self.duration).clamp(0.0, 1.0) as f32
            } else {
                0.0
            });
        }
    }
    fn shuffle_toggle(&mut self) {
        if self.queue.home {
            self.notice("На главной включено перемешивание. Для воспроизведения по порядку откройте альбом или плейлист.");
            return;
        }
        self.settings.shuffle = !self.settings.shuffle;
        self.queue.set_shuffle(self.settings.shuffle);
        self.settings_ui();
        self.dirty = true;
    }
    fn modal(&mut self, mode: i32) {
        let Some(ui) = self.ui() else {
            return;
        };
        let name = if let Some(Detail::Playlist(id)) = self.detail {
            self.playlists
                .iter()
                .find(|playlist| playlist.id == id)
                .map(|playlist| playlist.name.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let (title, description) = match mode {
            1 => (
                "Новый плейлист".to_owned(),
                "Введите название плейлиста.".to_owned(),
            ),
            2 => (
                "Переименовать плейлист".to_owned(),
                "Введите новое название.".to_owned(),
            ),
            3 => (
                "Удалить плейлист?".to_owned(),
                format!("\"{name}\" будет удалён. Музыкальные файлы останутся на месте."),
            ),
            _ => (
                "Добавить в плейлист".to_owned(),
                "Выберите плейлист.".to_owned(),
            ),
        };
        ui.set_modal(mode);
        ui.set_modal_error("".into());
        ui.set_modal_title(title.into());
        ui.set_modal_description(description.into());
        ui.set_modal_name(if mode == 2 { name.into() } else { "".into() });
    }
    fn modal_close(&mut self) {
        self.pending_add = None;
        if let Some(ui) = self.ui() {
            ui.set_modal(0);
        }
    }
    fn modal_submit(&mut self, name: &str) {
        let mode = self.ui().map(|ui| ui.get_modal()).unwrap_or_default();
        match mode {
            1 => {
                let result = manager::create(&self.database, name);
                let Some(id) = self.report(result) else {
                    return;
                };
                if let Some(track) = self.pending_add.take() {
                    let result = manager::add(&self.database, id, track);
                    self.report(result);
                }
                self.detail = Some(Detail::Playlist(id));
                if let Some(ui) = self.ui() {
                    ui.set_page(1);
                    ui.set_library_tab(1);
                }
            }
            2 => {
                if let Some(Detail::Playlist(id)) = self.detail {
                    let result = manager::rename(&self.database, id, name);
                    if self.report(result).is_none() {
                        return;
                    }
                }
            }
            3 => {
                if let Some(Detail::Playlist(id)) = self.detail {
                    let result = manager::delete(&self.database, id);
                    if self.report(result).is_none() {
                        return;
                    }
                    self.detail = None;
                }
            }
            _ => return,
        }
        self.modal_close();
        self.reload_playlists();
        self.detail_ui();
    }
    fn add_track(&mut self, index: i32) {
        if let Some(id) = self.detail_ids.get(index as usize).copied() {
            self.pending_add = Some(id);
            self.reload_playlists();
            self.modal(4);
        }
    }
    fn choice(&mut self, index: i32) {
        let Some(track) = self.pending_add else {
            return;
        };
        let Some(playlist) = self.playlists.get(index as usize) else {
            return;
        };
        let result = manager::add(&self.database, playlist.id, track);
        if self.report(result).is_some() {
            self.modal_close();
            self.reload_playlists();
            self.detail_ui();
            self.notice("Композиция добавлена в плейлист.");
        }
    }
    fn remove_track(&mut self, index: i32) {
        if let (Some(Detail::Playlist(playlist)), Some(track)) = (
            self.detail.clone(),
            self.detail_ids.get(index as usize).copied(),
        ) {
            let result = manager::remove(&self.database, playlist, track);
            if self.report(result).is_some() {
                self.reload_playlists();
                self.detail_ui();
            }
        }
    }
    fn restore(&mut self) {
        let resume = self
            .database
            .get("resume")
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str::<Resume>(&value).ok());
        if let Some(resume) = resume {
            let mut ids: Vec<_> = resume
                .tracks
                .into_iter()
                .filter(|id| self.library.track(*id).is_some())
                .collect();
            if self.library.track(resume.track).is_some() {
                if !ids.contains(&resume.track) {
                    ids.push(resume.track);
                }
                self.queue
                    .start(ids, Some(resume.track), self.settings.shuffle, resume.home);
                self.settings.shuffle = self.queue.shuffle;
                let duration = self
                    .library
                    .track(resume.track)
                    .map(|track| track.duration)
                    .unwrap_or(0.0);
                let position = if resume.position.is_finite() {
                    resume.position.clamp(0.0, (duration - 0.2).max(0.0))
                } else {
                    0.0
                };
                self.load(resume.track, position, false);
                self.settings_ui();
            }
        }
    }
    fn tick(&mut self) {
        let scan_result = self.scan.as_ref().map(|scan| scan.try_recv());
        if let Some(result) = scan_result {
            match result {
                Ok(result) => {
                    self.scan = None;
                    if let Some(ui) = self.ui() {
                        ui.set_scanning(false);
                    }
                    match result {
                        Ok(library) => {
                            let warnings = library.warnings.clone();
                            self.library = library;
                            self.library_ui();
                            if !warnings.is_empty() {
                                // Store the complete scan report; the small banner does not flood the interface.
                                let _ = std::fs::write(
                                    self.cache
                                        .parent()
                                        .unwrap_or(&self.cache)
                                        .join("library-report.txt"),
                                    warnings.join("\n"),
                                );
                                self.notice(if self.library.tracks.is_empty() { warnings[0].clone() } else { format!("Загружено {} композиций. Проблемных файлов или папок: {}. Подробности в library-report.txt.", self.library.tracks.len(), warnings.len()) });
                            }
                            if self.first_scan {
                                self.first_scan = false;
                                self.restore();
                            }
                        }
                        Err(error) => {
                            self.notice(format!("Не удалось обновить медиатеку: {error}"))
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan = None;
                    if let Some(ui) = self.ui() {
                        ui.set_scanning(false);
                    }
                    self.notice("Поиск музыки прерван. Обновите медиатеку.");
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        while let Ok(event) = self.player.events.try_recv() {
            match event {
                Event::Ready {
                    token,
                    duration,
                    position,
                } if token == self.token => {
                    if duration > 0.0 {
                        self.duration = duration;
                    }
                    self.position = position;
                    self.progress_ui();
                }
                Event::Progress { token, position } if token == self.token => {
                    self.position = position;
                    self.progress_ui();
                }
                Event::Ended(token) if token == self.token => self.next(false),
                Event::Error { token, message } if token == self.token => {
                    self.notice(message);
                    if let Some(id) = self.queue.current {
                        self.failed.insert(id);
                    }
                    self.playing = false;
                    if let Some(ui) = self.ui() {
                        ui.set_playing(false);
                    }
                    // Skip a broken track, but stop if this entire queue is broken.
                    for _ in 0..self.queue.tracks.len() {
                        let Some(id) = self.queue.advance(self.settings.repeat, true) else {
                            break;
                        };
                        if !self.failed.contains(&id) {
                            self.load(id, 0.0, true);
                            break;
                        }
                    }
                }
                Event::Fatal(message) => {
                    self.audio_available = false;
                    self.playing = false;
                    self.player.send(Command::Playing(false));
                    if let Some(ui) = self.ui() {
                        ui.set_playing(false);
                    }
                    self.notice(message);
                }
                _ => {} // Ignore events for a track replaced by a newer Load command.
            }
        }
        if self.settings.snow {
            if let Some(ui) = self.ui() {
                ui.set_snow_phase(self.snow_start.elapsed().as_secs_f32());
            }
        }
        if Instant::now() >= self.notice_until {
            if let Some(ui) = self.ui() {
                if !ui.get_notice().is_empty() {
                    ui.set_notice("".into());
                }
            }
        }
        if (self.dirty && self.last_save.elapsed() > Duration::from_millis(600))
            || (self.playing && self.last_save.elapsed() > Duration::from_secs(5))
        {
            self.save();
        }
    }
}

pub fn run() -> Result<()> {
    let executable = std::env::current_exe()?;
    // Cargo development runs use the checked-out songs folder; distributed builds use the executable folder.
    let songs = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("songs")
    } else {
        executable
            .parent()
            .context("Не удалось определить папку приложения")?
            .join("songs")
    };
    let data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .context("Переменная LOCALAPPDATA недоступна")?
        .join("ZelenkaMusic");
    let (window, state) = create(songs, data)?;
    let timer = Timer::default();
    let timer_state = state.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let Ok(mut app) = timer_state.try_borrow_mut() {
            app.tick();
        }
    });
    window.run()?;
    timer.stop();
    state.borrow_mut().save();
    Ok(())
}

fn create(songs: PathBuf, data: PathBuf) -> Result<(AppWindow, Rc<RefCell<App>>)> {
    let window = AppWindow::new()?;
    let database = Database::open(&data.join("zelenka.db"))?;
    let settings = Settings::load(&database);
    let player = Player::new();
    player.send(Command::Volume(settings.volume));
    let mut images = HashMap::new();
    images.insert(PathBuf::from("__placeholder__"), window.get_artwork());
    window.set_songs_path(songs.display().to_string().into());
    let state = Rc::new(RefCell::new(App {
        window: window.as_weak(),
        database,
        settings,
        player,
        library: Library::default(),
        playlists: vec![],
        queue: Queue::default(),
        songs,
        cache: data.join("artwork"),
        scan: None,
        detail: None,
        detail_ids: vec![],
        images,
        pending_add: None,
        position: 0.0,
        duration: 0.0,
        playing: false,
        token: 0,
        audio_available: true,
        failed: HashSet::new(),
        notice_until: Instant::now(),
        last_save: Instant::now(),
        dirty: false,
        snow_start: Instant::now(),
        first_scan: true,
    }));
    macro_rules! wire {
        ($callback:ident, |$app:ident $(, $arg:ident)*| $body:block) => {{
            let shared = state.clone();
            window.$callback(move |$($arg),*| { if let Ok(mut borrowed) = shared.try_borrow_mut() { let $app = &mut *borrowed; $body } });
        }};
    }
    wire!(on_navigate, |app, page| {
        if let Some(ui) = app.ui() {
            ui.set_page(page.clamp(0, 2));
        }
    });
    wire!(on_home_play, |app| {
        app.home_play();
    });
    wire!(on_toggle, |app| {
        app.toggle();
    });
    wire!(on_previous, |app| {
        app.previous();
    });
    wire!(on_next, |app| {
        app.next(true);
    });
    wire!(on_seek, |app, value| {
        app.seek(value);
    });
    wire!(on_volume_change, |app, value| {
        app.settings.volume = value.clamp(0.0, 1.0);
        app.player.send(Command::Volume(app.settings.volume));
        app.settings_ui();
        app.dirty = true;
    });
    wire!(on_shuffle_change, |app| {
        app.shuffle_toggle();
    });
    wire!(on_repeat_change, |app| {
        app.settings.repeat = app.settings.repeat.next();
        app.settings_ui();
        app.dirty = true;
    });
    wire!(on_album_open, |app, index| {
        app.open_album(index);
    });
    wire!(on_playlist_open, |app, index| {
        app.open_playlist(index);
    });
    wire!(on_detail_back, |app| {
        app.detail = None;
        app.detail_ui();
    });
    wire!(on_collection_play, |app, shuffle| {
        app.start(app.detail_ids.clone(), None, shuffle, false);
    });
    wire!(on_track_play, |app, index| {
        if let Some(id) = app.detail_ids.get(index as usize).copied() {
            app.start(
                app.detail_ids.clone(),
                Some(id),
                app.settings.shuffle,
                false,
            );
        }
    });
    wire!(on_track_add, |app, index| {
        app.add_track(index);
    });
    wire!(on_track_remove, |app, index| {
        app.remove_track(index);
    });
    wire!(on_playlist_create, |app| {
        app.modal(1);
    });
    wire!(on_playlist_rename, |app| {
        app.modal(2);
    });
    wire!(on_playlist_delete, |app| {
        app.modal(3);
    });
    wire!(on_modal_submit, |app, name| {
        app.modal_submit(name.as_str());
    });
    wire!(on_modal_close, |app| {
        app.modal_close();
    });
    wire!(on_choice_select, |app, index| {
        app.choice(index);
    });
    wire!(on_snow_change, |app, value| {
        app.settings.snow = value;
        app.settings_ui();
        app.dirty = true;
    });
    wire!(on_rescan, |app| {
        app.start_scan();
    });
    state.borrow().settings_ui();
    state.borrow_mut().start_scan();
    Ok((window, state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PlatformError, WindowAdapter,
    };

    struct Headless {
        start: Instant,
        adapters: Rc<RefCell<Vec<Rc<MinimalSoftwareWindow>>>>,
    }
    impl Platform for Headless {
        fn create_window_adapter(
            &self,
        ) -> std::result::Result<Rc<dyn WindowAdapter>, PlatformError> {
            let adapter = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
            self.adapters.borrow_mut().push(adapter.clone());
            Ok(adapter)
        }
        fn duration_since_start(&self) -> Duration {
            self.start.elapsed()
        }
    }
    fn pump(state: &Rc<RefCell<App>>, duration: Duration) {
        let start = Instant::now();
        while start.elapsed() < duration {
            slint::platform::update_timers_and_animations();
            state.borrow_mut().tick();
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn screenshot(window: &AppWindow, name: &str) -> Result<()> {
        let pixels = window.window().take_snapshot()?;
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/verification")
            .join(name);
        image::save_buffer(
            file,
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
        )?;
        Ok(())
    }

    fn pointer(window: &AppWindow, event: slint::platform::WindowEvent) {
        window.window().dispatch_event(event);
    }
    fn mouse(window: &AppWindow, x: f32, y: f32, down: bool) {
        let position = slint::LogicalPosition::new(x, y);
        let button = slint::platform::PointerEventButton::Left;
        pointer(
            window,
            if down {
                slint::platform::WindowEvent::PointerPressed { position, button }
            } else {
                slint::platform::WindowEvent::PointerReleased { position, button }
            },
        );
    }

    #[test]
    fn pointer_sliders_and_incremental_snow() -> Result<()> {
        let adapters = Rc::new(RefCell::new(Vec::new()));
        slint::platform::set_platform(Box::new(Headless {
            start: Instant::now(),
            adapters: adapters.clone(),
        }))
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let window = AppWindow::new()?;
        window
            .window()
            .set_size(slint::LogicalSize::new(1120.0, 820.0));
        window.show()?;
        window.set_has_track(true);
        let last_volume = Rc::new(std::cell::Cell::new(-1.0_f32));
        let last_seek = Rc::new(std::cell::Cell::new(-1.0_f32));
        let seek_calls = Rc::new(std::cell::Cell::new(0));
        let volume = last_volume.clone();
        let seek = last_seek.clone();
        let weak = window.as_weak();
        window.on_volume_change(move |value| {
            volume.set(value);
            weak.upgrade().unwrap().set_volume(value);
        });
        let weak = window.as_weak();
        let calls = seek_calls.clone();
        window.on_seek(move |value| {
            calls.set(calls.get() + 1);
            seek.set(value);
            weak.upgrade().unwrap().set_progress(value);
        });
        let adapter = adapters.borrow()[0].clone();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 1120 * 820];
        adapter.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 1120);
        });
        for snow in [false, true] {
            window.set_snow(snow);
            adapter.draw_if_needed(|renderer| {
                renderer.render(&mut pixels, 1120);
            });
            mouse(&window, 1007.0, 688.0, true);
            mouse(&window, 1007.0, 688.0, false);
            anyhow::ensure!(
                (last_volume.get() - 0.5).abs() < 0.08,
                "Volume pointer click failed with snow={snow}: {}",
                last_volume.get()
            );
            mouse(&window, 565.0, 710.0, true);
            mouse(&window, 565.0, 710.0, false);
            anyhow::ensure!(
                (last_seek.get() - 0.5).abs() < 0.08,
                "Seek pointer click failed with snow={snow}: {}",
                last_seek.get()
            );
            let rendered = window.window().take_snapshot()?;
            anyhow::ensure!(
                rendered.as_slice()[710 * 1120 + 405].g > 120,
                "Seek fill must begin at the left end, not in the middle"
            );
            anyhow::ensure!(
                rendered.as_slice()[688 * 1120 + 970].g > 120,
                "Volume fill must begin at the left end"
            );
            seek_calls.set(0);
            mouse(&window, 390.0, 710.0, true);
            for step in 0..200 {
                pointer(
                    &window,
                    slint::platform::WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(390.0 + step as f32 * 1.7, 710.0),
                    },
                );
                window.set_progress(0.1); // Playback progress must not overwrite the dragged thumb.
            }
            anyhow::ensure!(
                seek_calls.get() == 0,
                "Dragging dispatched decoding requests before release"
            );
            mouse(&window, 730.0, 710.0, false);
            anyhow::ensure!(
                last_seek.get() > 0.85,
                "Seek drag failed with snow={snow}: {}",
                last_seek.get()
            );
            anyhow::ensure!(seek_calls.get() == 1, "A drag must commit exactly one seek");
            mouse(&window, 955.0, 688.0, true);
            pointer(
                &window,
                slint::platform::WindowEvent::PointerMoved {
                    position: slint::LogicalPosition::new(1055.0, 688.0),
                },
            );
            anyhow::ensure!(
                last_volume.get() > 0.85,
                "Volume did not update while dragging"
            );
            mouse(&window, 1055.0, 688.0, false);
        }
        // Windows may reuse the last buffer or alternate between two buffers. Exercise both.
        for page in 0..3 {
            window.set_page(page);
            for (mode, buffers) in [
                (RepaintBufferType::ReusedBuffer, 1),
                (RepaintBufferType::SwappedBuffers, 2),
            ] {
                let mut frames = [
                    vec![slint::Rgb8Pixel::default(); 1120 * 820],
                    vec![slint::Rgb8Pixel::default(); 1120 * 820],
                ];
                let mut last = 0;
                for frame in 0..32 {
                    last = frame % buffers;
                    window.set_snow_phase(frame as f32 * 1.31 + page as f32 * 71.0);
                    slint::platform::update_timers_and_animations();
                    adapter.draw_if_needed(|renderer| {
                        // Fresh allocated buffers must be painted completely before reuse.
                        renderer.set_repaint_buffer_type(if frame < buffers {
                            RepaintBufferType::NewBuffer
                        } else {
                            mode
                        });
                        renderer.render(&mut frames[last], 1120);
                    });
                }
                let full = window.window().take_snapshot()?;
                let mismatches = frames[last]
                    .iter()
                    .zip(full.as_slice())
                    .filter(|(a, b)| {
                        a.r.abs_diff(b.r) > 3 || a.g.abs_diff(b.g) > 3 || a.b.abs_diff(b.b) > 3
                    })
                    .count();
                anyhow::ensure!(mismatches < 30, "Snowfall corrupts incremental rendering on page {page}, buffers={buffers}: {mismatches} pixels");
            }
        }
        for (width, height) in [(940.0, 760.0), (1280.0, 900.0)] {
            window
                .window()
                .set_size(slint::LogicalSize::new(width, height));
            let _ = window.window().take_snapshot()?; // Recalculate layout after resizing.
            let bar_width = width - 60.0;
            let seek_start = 30.0 + bar_width * 0.29 + 41.0;
            let seek_width = bar_width * 0.43 - 86.0;
            mouse(
                &window,
                seek_start + seek_width * 0.65,
                height - 110.0,
                true,
            );
            mouse(
                &window,
                seek_start + seek_width * 0.65,
                height - 110.0,
                false,
            );
            anyhow::ensure!(
                (last_seek.get() - 0.65).abs() < 0.04,
                "Seek hit area moved incorrectly after resize"
            );
            mouse(&window, width - 170.0 + 114.0 * 0.35, height - 132.0, true);
            mouse(&window, width - 170.0 + 114.0 * 0.35, height - 132.0, false);
            anyhow::ensure!(
                (last_volume.get() - 0.35).abs() < 0.04,
                "Volume hit area moved incorrectly after resize"
            );
        }
        window.hide()?;
        Ok(())
    }

    /// Uses the actual UI callbacks, software renderer, scanner, database and Windows audio device.
    /// Run explicitly because it needs the user's real library and an output device.
    #[test]
    #[ignore = "requires the local songs library and a Windows audio output device"]
    fn real_ui_and_audio_smoke() -> Result<()> {
        slint::platform::set_platform(Box::new(Headless {
            start: Instant::now(),
            adapters: Rc::default(),
        }))
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let data = root.join("target/verification/ui-smoke");
        std::fs::create_dir_all(&data)?;
        let (window, state) = create(root.join("songs"), data)?;
        window
            .window()
            .set_size(slint::LogicalSize::new(1120.0, 820.0));
        window.show()?;
        let _ = window.window().take_snapshot()?;
        mouse(&window, 1007.0, 688.0, true);
        mouse(&window, 1007.0, 688.0, false);
        anyhow::ensure!(
            (state.borrow().settings.volume - 0.5).abs() < 0.08,
            "The real volume control did not update the audio preference"
        );
        window.invoke_volume_change(0.0); // Exercise real output without playing sound during testing.
        let scan_started = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(90);
        while state.borrow().scan.is_some() && Instant::now() < deadline {
            pump(&state, Duration::from_millis(40));
        }
        println!(
            "Library available after {} ms",
            scan_started.elapsed().as_millis()
        );
        anyhow::ensure!(
            window.get_has_library(),
            "No music discovered: {}",
            window.get_notice()
        );
        anyhow::ensure!(
            state.borrow().audio_available,
            "Windows audio output unavailable: {}",
            window.get_notice()
        );
        screenshot(&window, "ui-home.png")?;
        anyhow::ensure!(
            !window.get_playing(),
            "Restored playback must remain paused"
        );
        window.invoke_home_play();
        pump(&state, Duration::from_millis(700));
        anyhow::ensure!(
            window.get_playing() && state.borrow().position > 0.0,
            "Audio did not advance: {}",
            window.get_notice()
        );
        anyhow::ensure!(window.get_shuffle(), "Home must shuffle");
        window.invoke_navigate(1);
        pump(&state, Duration::from_millis(220));
        anyhow::ensure!(window.get_playing(), "Navigation paused playback");
        screenshot(&window, "ui-library.png")?;
        for extension in ["mp3", "flac"] {
            let track = state
                .borrow()
                .library
                .tracks
                .iter()
                .find(|track| track.path.extension().is_some_and(|ext| ext == extension))
                .cloned()
                .context("Missing codec fixture")?;
            let album = state
                .borrow()
                .library
                .albums
                .iter()
                .position(|album| album.key == track.album_key)
                .unwrap();
            window.invoke_album_open(album as i32);
            window.invoke_collection_play(false);
            let index = state
                .borrow()
                .detail_ids
                .iter()
                .position(|id| *id == track.id)
                .unwrap();
            window.invoke_track_play(index as i32);
            pump(&state, Duration::from_millis(500));
            anyhow::ensure!(
                window.get_playing() && state.borrow().position > 0.0,
                "{extension} output failed: {}",
                window.get_notice()
            );
            window.invoke_toggle();
            pump(&state, Duration::from_millis(150));
            let paused = state.borrow().position;
            pump(&state, Duration::from_millis(200));
            anyhow::ensure!(
                (state.borrow().position - paused).abs() < 0.05,
                "Pause did not stop the audio clock"
            );
            let _ = window.window().take_snapshot()?;
            let token_before_drag = state.borrow().token;
            mouse(&window, 390.0, 710.0, true);
            for step in 0..100 {
                pointer(
                    &window,
                    slint::platform::WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(390.0 + step as f32 * 1.75, 710.0),
                    },
                );
            }
            anyhow::ensure!(
                state.borrow().token == token_before_drag,
                "Dragging repeatedly reset the decoder"
            );
            mouse(&window, 565.0, 710.0, false);
            pump(&state, Duration::from_millis(200));
            anyhow::ensure!(
                state.borrow().position >= state.borrow().duration * 0.49,
                "{extension} seek failed"
            );
            window.invoke_toggle();
            pump(&state, Duration::from_millis(300));
            anyhow::ensure!(window.get_playing(), "Resume failed");
        }
        screenshot(&window, "ui-album.png")?;
        // Seek to EOF to exercise automatic transitions and repeat-one through CPAL.
        if state.borrow().settings.repeat != crate::settings::Repeat::One {
            state.borrow_mut().settings.repeat = crate::settings::Repeat::One;
        }
        let before = state.borrow().queue.current;
        window.invoke_seek(1.0);
        pump(&state, Duration::from_secs(2));
        anyhow::ensure!(
            state.borrow().queue.current == before
                && state.borrow().position < 5.0
                && window.get_playing(),
            "Repeat-one transition failed: {}",
            window.get_notice()
        );
        state.borrow_mut().settings.repeat = crate::settings::Repeat::All;
        window.invoke_seek(1.0);
        pump(&state, Duration::from_secs(2));
        anyhow::ensure!(
            state.borrow().queue.current != before && window.get_playing(),
            "Automatic next track failed: {}",
            window.get_notice()
        );
        window.invoke_playlist_create();
        window.invoke_modal_submit("   ".into());
        anyhow::ensure!(
            window.get_modal() == 1 && !window.get_modal_error().is_empty(),
            "Invalid playlist names must show an error inside the dialog"
        );
        window.invoke_modal_submit("я".repeat(81).into());
        anyhow::ensure!(
            window.get_modal() == 1 && window.get_modal_error().contains("80"),
            "Overlong playlist names were accepted"
        );
        window.invoke_playlist_create();
        screenshot(&window, "ui-create-playlist.png")?;
        window.invoke_modal_submit("Тестовый плейлист".into());
        anyhow::ensure!(window.get_is_playlist(), "Playlist creation failed");
        let playlist = state.borrow().playlists[0].id;
        window.invoke_album_open(0);
        window.invoke_track_add(0);
        window.invoke_choice_select(0);
        window.invoke_playlist_open(0);
        anyhow::ensure!(
            state.borrow().detail_ids.len() == 1,
            "Playlist addition failed"
        );
        window.invoke_playlist_rename();
        window.invoke_modal_submit("Новый тестовый плейлист".into());
        anyhow::ensure!(
            window.get_detail_title() == "Новый тестовый плейлист",
            "Playlist rename failed"
        );
        screenshot(&window, "ui-playlist.png")?;
        window.invoke_track_remove(0);
        anyhow::ensure!(
            state.borrow().detail_ids.is_empty(),
            "Playlist removal failed"
        );
        window.invoke_playlist_delete();
        window.invoke_modal_submit("".into());
        anyhow::ensure!(
            !state
                .borrow()
                .playlists
                .iter()
                .any(|item| item.id == playlist),
            "Playlist deletion failed"
        );
        window.invoke_navigate(2);
        window.invoke_snow_change(true);
        pump(&state, Duration::from_millis(220));
        screenshot(&window, "ui-settings.png")?;
        window
            .window()
            .set_size(slint::LogicalSize::new(940.0, 760.0));
        window.invoke_navigate(0);
        pump(&state, Duration::from_millis(220));
        screenshot(&window, "ui-minimum.png")?;
        state.borrow_mut().save();
        anyhow::ensure!(
            Settings::load(&state.borrow().database).snow,
            "Snow preference not saved"
        );
        window.hide()?;
        let fixture = root.join("target/verification/error-fixture");
        std::fs::create_dir_all(&fixture)?;
        let good = state
            .borrow()
            .library
            .tracks
            .iter()
            .find(|track| track.path.extension().is_some_and(|ext| ext == "mp3"))
            .cloned()
            .unwrap();
        std::fs::copy(&good.path, fixture.join("valid.mp3"))?;
        std::fs::write(fixture.join("damaged.flac"), b"not an audio file")?;
        let result = scanner::scan(
            &fixture,
            &root.join("target/verification/error-art"),
            &root.join("target/verification/error.db"),
        )?;
        anyhow::ensure!(
            result.tracks.len() == 1 && result.warnings.len() == 1,
            "One damaged file prevented valid audio from loading"
        );
        let missing = scanner::scan(
            &fixture.join("missing"),
            &root.join("target/verification/error-art"),
            &root.join("target/verification/missing.db"),
        )?;
        anyhow::ensure!(
            missing.tracks.is_empty() && !missing.warnings.is_empty(),
            "Missing songs directory was not handled"
        );
        println!("Real MP3/FLAC output, seek, pause, EOF transitions, UI callbacks, playlists, snowfall and snapshots passed.");
        Ok(())
    }
}
