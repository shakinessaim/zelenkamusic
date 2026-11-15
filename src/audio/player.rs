use super::decoder::{AudioDecoder, Resampler};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, RingBuffer};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub enum Command {
    Load {
        token: u64,
        path: PathBuf,
        position: f64,
        playing: bool,
    },
    Playing(bool),
    Seek {
        token: u64,
        position: f64,
    },
    Volume(f32),
    Shutdown,
}
#[derive(Debug)]
pub enum Event {
    Ready {
        token: u64,
        duration: f64,
        position: f64,
    },
    Progress {
        token: u64,
        position: f64,
    },
    Ended(u64),
    Error {
        token: u64,
        message: String,
    },
    Fatal(String),
}
pub struct Player {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Player {
    pub fn new() -> Self {
        let (commands, receiver) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let worker = thread::Builder::new().name("zelenka-audio".into()).spawn(move || {
            if let Err(error) = run(receiver, sender.clone()) { let _ = sender.send(Event::Fatal(format!("Аудиовыход недоступен: {error:#}. Подключите устройство и перезапустите ZelenkaMusic."))); }
        }).expect("Cannot start audio thread");
        Self {
            commands,
            events,
            worker: Some(worker),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Frame {
    samples: [f32; 2],
    generation: u64,
}
struct Shared {
    playing: AtomicBool,
    volume: AtomicU32,
    generation: AtomicU64,
    rendered: AtomicU64,
}

fn stream<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: Consumer<Frame>,
    shared: Arc<Shared>,
    events: Sender<Event>,
) -> Result<cpal::Stream> {
    let channels = config.channels as usize;
    Ok(device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            let playing = shared.playing.load(Ordering::Acquire);
            let generation = shared.generation.load(Ordering::Acquire);
            let volume = f32::from_bits(shared.volume.load(Ordering::Relaxed));
            let mut rendered = 0;
            for target in output.chunks_mut(channels) {
                let frame = if playing {
                    loop {
                        match consumer.pop() {
                            Ok(frame) if frame.generation == generation => {
                                rendered += 1;
                                break frame.samples;
                            }
                            Ok(_) => continue,
                            Err(_) => break [0.0, 0.0],
                        }
                    }
                } else {
                    [0.0, 0.0]
                };
                for (channel, sample) in target.iter_mut().enumerate() {
                    let value = if channels == 1 {
                        (frame[0] + frame[1]) * 0.5
                    } else if channel < 2 {
                        frame[channel]
                    } else {
                        0.0
                    };
                    *sample = T::from_sample(value * volume);
                }
            }
            // A seek can reset the counter during a callback; do not add stale frames.
            if shared.generation.load(Ordering::Acquire) == generation {
                shared.rendered.fetch_add(rendered, Ordering::Release);
            }
        },
        move |error| {
            let _ = events.send(Event::Fatal(format!(
                "Ошибка аудиоустройства: {error}. Проверьте аудиовыход Windows и перезапустите приложение."
            )));
        },
        None,
    )?)
}

fn run(commands: Receiver<Command>, events: Sender<Event>) -> Result<()> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| anyhow!("Аудиоустройство по умолчанию не найдено"))?;
    let supported = device.default_output_config()?;
    let config: cpal::StreamConfig = supported.clone().into();
    let rate = config.sample_rate.0;
    let capacity = rate as usize / 2;
    let (mut producer, consumer) = RingBuffer::new(capacity);
    let shared = Arc::new(Shared {
        playing: AtomicBool::new(false),
        volume: AtomicU32::new(0.75f32.to_bits()),
        generation: AtomicU64::new(1),
        rendered: AtomicU64::new(0),
    });
    let output = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            stream::<f32>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::F64 => {
            stream::<f64>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::I8 => {
            stream::<i8>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::I16 => {
            stream::<i16>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::I32 => {
            stream::<i32>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::I64 => {
            stream::<i64>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::U8 => {
            stream::<u8>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::U16 => {
            stream::<u16>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::U32 => {
            stream::<u32>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        cpal::SampleFormat::U64 => {
            stream::<u64>(&device, &config, consumer, shared.clone(), events.clone())?
        }
        _ => return Err(anyhow!("Формат аудиовыхода не поддерживается")),
    };
    output.play()?;
    let mut decoder: Option<AudioDecoder> = None;
    let mut resampler = Resampler::new();
    let mut pending = Vec::<[f32; 2]>::new();
    let mut offset = 0;
    let mut base = 0.0;
    let mut ended = false;
    let mut tick = Instant::now();
    let mut token = 0;
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Shutdown => return Ok(()),
                Command::Volume(value) => shared
                    .volume
                    .store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed),
                Command::Playing(value) => shared
                    .playing
                    .store(value && decoder.is_some(), Ordering::Release),
                Command::Load {
                    token: next_token,
                    path,
                    position,
                    playing,
                } => {
                    token = next_token;
                    shared.playing.store(false, Ordering::Release);
                    shared.generation.fetch_add(1, Ordering::AcqRel);
                    shared.rendered.store(0, Ordering::Release);
                    pending.clear();
                    offset = 0;
                    ended = false;
                    base = 0.0;
                    resampler = Resampler::new();
                    match AudioDecoder::open(&path).and_then(|mut next| {
                        if position > 0.0 {
                            base = next.seek(position)?;
                        }
                        Ok(next)
                    }) {
                        Ok(next) => {
                            let _ = events.send(Event::Ready {
                                token,
                                duration: next.duration,
                                position: base,
                            });
                            decoder = Some(next);
                            shared.playing.store(playing, Ordering::Release);
                        }
                        Err(error) => {
                            decoder = None;
                            let _ = events.send(Event::Error {
                                token,
                                message: format!(
                                    "Не удалось воспроизвести {}: {error:#}",
                                    path.display()
                                ),
                            });
                        }
                    }
                }
                Command::Seek {
                    token: next_token,
                    position,
                } => {
                    token = next_token;
                    if let Some(decoder) = &mut decoder {
                        let playing = shared.playing.swap(false, Ordering::AcqRel);
                        shared.generation.fetch_add(1, Ordering::AcqRel);
                        shared.rendered.store(0, Ordering::Release);
                        pending.clear();
                        offset = 0;
                        ended = false;
                        resampler = Resampler::new();
                        match decoder.seek(position) {
                            Ok(position) => {
                                base = position;
                                let _ = events.send(Event::Progress {
                                    token,
                                    position: base,
                                });
                            }
                            Err(error) => {
                                let _ = events.send(Event::Error {
                                    token,
                                    message: format!("Не удалось перемотать: {error:#}"),
                                });
                            }
                        }
                        shared.playing.store(playing, Ordering::Release);
                    }
                }
            }
        }
        if let Some(active_decoder) = decoder
            .as_mut()
            .filter(|_| shared.playing.load(Ordering::Acquire))
        {
            if offset == pending.len() && !ended {
                match active_decoder.decode_next() {
                    Ok(Some(chunk)) => {
                        pending = resampler.convert(chunk, rate);
                        offset = 0;
                    }
                    Ok(None) => {
                        ended = true;
                    }
                    Err(error) => {
                        shared.playing.store(false, Ordering::Release);
                        let _ = events.send(Event::Error {
                            token,
                            message: format!("Ошибка декодирования: {error:#}"),
                        });
                    }
                }
            }
            let generation = shared.generation.load(Ordering::Acquire);
            while offset < pending.len() {
                if producer
                    .push(Frame {
                        samples: pending[offset],
                        generation,
                    })
                    .is_err()
                {
                    break;
                }
                offset += 1;
            }
            if ended && offset == pending.len() && producer.slots() == capacity {
                shared.playing.store(false, Ordering::Release);
                let _ = events.send(Event::Ended(token));
            }
        }
        if tick.elapsed() >= Duration::from_millis(100) {
            let _ = events.send(Event::Progress {
                token,
                position: base + shared.rendered.load(Ordering::Acquire) as f64 / rate as f64,
            });
            tick = Instant::now();
        }
        // Decode ahead only to the bounded half-second ring; poll controls at least every 4 ms.
        if !shared.playing.load(Ordering::Acquire) || producer.slots() == 0 || ended {
            thread::sleep(Duration::from_millis(4));
        }
    }
}
