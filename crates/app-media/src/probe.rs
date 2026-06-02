use std::fs;
use std::time::Duration;

use anyhow::{Context, Result};
use ffmpeg_next::{
    codec,
    format,
    media::Type,
    DictionaryRef,
    Rational,
};

use crate::ffmpeg_util::init_ffmpeg;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaProbeResult {
    pub metadata: crate::MediaFileMetadata,
}

pub fn probe_media(source: &crate::MediaSource) -> Result<MediaProbeResult> {
    let path = source.path().clone();
    let file_size = fs::metadata(&path).ok().map(|meta| meta.len());
    init_ffmpeg()?;

    let mut metadata = crate::MediaFileMetadata {
        source_path: Some(path.clone()),
        file_size,
        ..crate::MediaFileMetadata::default()
    };

    let input = format::input(&path)
        .with_context(|| format!("failed to open media input: {}", path.display()))?;
    metadata.duration = duration_from_micros(input.duration());
    let container_metadata = input.metadata();

    if let Some(audio_stream) = input.streams().best(Type::Audio) {
        let codec_context = codec::context::Context::from_parameters(audio_stream.parameters())
            .context("failed to build audio codec context")?;
        let audio_decoder = codec_context
            .decoder()
            .audio()
            .context("failed to open audio decoder for probe")?;

        metadata.audio = Some(crate::AudioFileMetadata {
            duration: stream_duration(audio_stream.duration(), audio_stream.time_base())
                .or(metadata.duration),
            title: metadata_tag(&container_metadata, &["title"]),
            artist: metadata_tag(&container_metadata, &["artist", "album_artist", "albumartist"]),
            album: metadata_tag(&container_metadata, &["album"]),
            codec: codec_name(audio_decoder.id()),
            sample_rate: Some(audio_decoder.rate()),
            channels: Some(audio_decoder.channels() as u16),
            bitrate_kbps: bitrate_kbps(audio_decoder.bit_rate()),
            file_size,
        });
    }

    if let Some(video_stream) = input.streams().best(Type::Video) {
        let codec_context = codec::context::Context::from_parameters(video_stream.parameters())
            .context("failed to build video codec context")?;
        let video_decoder = codec_context
            .decoder()
            .video()
            .context("failed to open video decoder for probe")?;

        let frame_rate = video_stream.avg_frame_rate();
        metadata.video = Some(crate::VideoFileMetadata {
            duration: stream_duration(video_stream.duration(), video_stream.time_base())
                .or(metadata.duration),
            codec: codec_name(video_decoder.id()),
            width: Some(video_decoder.width()),
            height: Some(video_decoder.height()),
            frame_rate_milli: frame_rate_milli(frame_rate),
            bitrate_kbps: bitrate_kbps(video_decoder.bit_rate()),
            file_size,
            has_audio: metadata.audio.is_some(),
        });
    }

    Ok(MediaProbeResult {
        metadata,
    })
}

fn duration_from_micros(value: i64) -> Option<Duration> {
    if value > 0 {
        Some(Duration::from_micros(value as u64))
    } else {
        None
    }
}

fn stream_duration(duration: i64, time_base: Rational) -> Option<Duration> {
    if duration <= 0 {
        return None;
    }

    let numerator = f64::from(time_base.numerator());
    let denominator = f64::from(time_base.denominator());
    if denominator <= 0.0 {
        return None;
    }

    let secs = duration as f64 * numerator / denominator;
    if secs.is_finite() && secs > 0.0 {
        Some(Duration::from_secs_f64(secs))
    } else {
        None
    }
}

fn bitrate_kbps(bit_rate: usize) -> Option<u32> {
    if bit_rate == 0 {
        None
    } else {
        Some((bit_rate / 1000) as u32)
    }
}

fn frame_rate_milli(rate: Rational) -> Option<u32> {
    let denominator = f64::from(rate.denominator());
    if denominator <= 0.0 {
        return None;
    }

    let fps = f64::from(rate.numerator()) / denominator;
    if fps.is_finite() && fps > 0.0 {
        Some((fps * 1000.0).round() as u32)
    } else {
        None
    }
}

fn codec_name(id: codec::Id) -> Option<String> {
    let name = id.name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn metadata_tag(dict: &DictionaryRef<'_>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = dict.get(key) {
            let value = value.to_string();
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }

    None
}
