use anyhow::{anyhow, Context, Result};
use std::{fs::File, path::Path};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder, DecoderOptions},
    errors::Error,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

pub struct AudioDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track: u32,
    pub duration: f64,
    failures: usize,
    discard_frames: usize,
}

pub struct Chunk {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
}

impl AudioDecoder {
    pub fn open(path: &Path) -> Result<Self> {
        let source = MediaSourceStream::new(
            Box::new(File::open(path).context("Не удалось открыть композицию")?),
            Default::default(),
        );
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        let probe = symphonia::default::get_probe()
            .format(
                &hint,
                source,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .context("Аудиофайл повреждён или не поддерживается")?;
        let format = probe.format;
        let track = format
            .default_track()
            .ok_or_else(|| anyhow!("Звуковая дорожка не найдена"))?;
        let duration = match (track.codec_params.n_frames, track.codec_params.sample_rate) {
            (Some(frames), Some(rate)) if rate > 0 => frames as f64 / rate as f64,
            _ => 0.0,
        };
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        let id = track.id;
        Ok(Self {
            format,
            decoder,
            track: id,
            duration,
            failures: 0,
            discard_frames: 0,
        })
    }

    pub fn seek(&mut self, seconds: f64) -> Result<f64> {
        let result = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: Time::from(seconds.max(0.0)),
                track_id: Some(self.track),
            },
        )?;
        self.decoder.reset();
        self.failures = 0;
        // MP3/FLAC can seek to a packet before the requested time. Discard the prefix.
        let params = &self
            .format
            .tracks()
            .iter()
            .find(|track| track.id == self.track)
            .ok_or_else(|| anyhow!("Звуковая дорожка недоступна"))?
            .codec_params;
        self.discard_frames = match (params.time_base, params.sample_rate) {
            (Some(base), Some(rate)) => {
                let actual = base.calc_time(result.actual_ts);
                ((seconds - (actual.seconds as f64 + actual.frac)).max(0.0) * rate as f64).round()
                    as usize
            }
            _ => 0,
        };
        Ok(seconds)
    }

    pub fn decode_next(&mut self) -> Result<Option<Chunk>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None)
                }
                Err(error) => return Err(error.into()),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    self.failures = 0;
                    decoded
                }
                Err(Error::DecodeError(_)) if self.failures < 12 => {
                    self.failures += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let channels = decoded.spec().channels.count();
            let rate = decoded.spec().rate;
            if channels == 0 || rate == 0 {
                return Err(anyhow!(
                    "Некорректное количество каналов или частота дискретизации"
                ));
            }
            let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            buffer.copy_interleaved_ref(decoded);
            let skipped = self.discard_frames.min(buffer.samples().len() / channels);
            self.discard_frames -= skipped;
            let samples = buffer.samples()[skipped * channels..].to_vec();
            if samples.is_empty() {
                continue;
            }
            return Ok(Some(Chunk {
                samples,
                channels,
                rate,
            }));
        }
    }
}

/// Streaming linear interpolation; carries one frame across packet boundaries.
#[derive(Default)]
pub struct Resampler {
    phase: f64,
    previous: Option<[f32; 2]>,
}
impl Resampler {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            previous: None,
        }
    }
    pub fn convert(&mut self, chunk: Chunk, output_rate: u32) -> Vec<[f32; 2]> {
        let mut frames = Vec::with_capacity(chunk.samples.len() / chunk.channels + 1);
        if let Some(previous) = self.previous {
            frames.push(previous);
        }
        for frame in chunk.samples.chunks_exact(chunk.channels) {
            // MP3 and FLAC stereo/mono are preserved. Multichannel files receive a stereo downmix.
            let left = frame[0];
            let right = *frame.get(1).unwrap_or(&left);
            let center = frame.get(2).copied().unwrap_or(0.0) * 0.707;
            let surround_left = frame.get(4).copied().unwrap_or(0.0) * 0.5;
            let surround_right = frame.get(5).copied().unwrap_or(0.0) * 0.5;
            frames.push([
                (left + center + surround_left).clamp(-1.0, 1.0),
                (right + center + surround_right).clamp(-1.0, 1.0),
            ]);
        }
        let step = chunk.rate as f64 / output_rate as f64;
        let mut output = Vec::with_capacity((frames.len() as f64 / step).ceil() as usize);
        while self.phase + 1.0 < frames.len() as f64 {
            let index = self.phase as usize;
            let mix = (self.phase - index as f64) as f32;
            output.push([
                frames[index][0] * (1.0 - mix) + frames[index + 1][0] * mix,
                frames[index][1] * (1.0 - mix) + frames[index + 1][1] * mix,
            ]);
            self.phase += step;
        }
        if !frames.is_empty() {
            self.phase -= (frames.len() - 1) as f64;
            self.previous = frames.last().copied();
        }
        output
    }
}
