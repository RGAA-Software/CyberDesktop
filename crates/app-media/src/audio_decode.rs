use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;

use crate::ffmpeg_util::init_ffmpeg;

#[derive(Debug)]
pub struct AudioChunk {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

pub fn decode_audio_file<F>(path: &Path, cancel_decode: &AtomicBool, mut on_chunk: F) -> Result<()>
where
    F: FnMut(AudioChunk),
{
    init_ffmpeg()?;

    let mut input = ffmpeg::format::input(path)
        .with_context(|| format!("open {}", path.display()))?;
    let audio_stream = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or_else(|| anyhow::anyhow!("no default audio track"))?;
    let stream_index = audio_stream.index();
    let codec_context = ffmpeg::codec::context::Context::from_parameters(audio_stream.parameters())
        .context("create ffmpeg codec context")?;
    let mut decoder = codec_context.decoder().audio().context("open ffmpeg decoder")?;

    let src_layout = decoder_channel_layout(&decoder);
    let dst_layout = src_layout;
    let dst_rate = decoder.rate();
    let dst_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);
    let mut resampler = ffmpeg::software::resampling::Context::get(
        decoder.format(),
        src_layout,
        dst_rate,
        dst_format,
        dst_layout,
        dst_rate,
    )
    .context("create ffmpeg audio resampler")?;

    for (stream, packet) in input.packets() {
        if cancel_decode.load(Ordering::Relaxed) {
            return Ok(());
        }
        if stream.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet).context("send ffmpeg audio packet")?;
        receive_decoded_audio_frames(&mut decoder, &mut resampler, &mut on_chunk)?;
    }

    decoder.send_eof().context("send ffmpeg audio eof")?;
    receive_decoded_audio_frames(&mut decoder, &mut resampler, &mut on_chunk)?;
    flush_resampler(&mut resampler, &mut on_chunk)?;

    Ok(())
}

fn decoder_channel_layout(decoder: &ffmpeg::decoder::Audio) -> ffmpeg::ChannelLayout {
    let layout = decoder.channel_layout();
    if layout.is_empty() {
        ffmpeg::ChannelLayout::default(i32::from(decoder.channels().max(1)))
    } else {
        layout
    }
}

fn receive_decoded_audio_frames<F>(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ffmpeg::software::resampling::Context,
    on_chunk: &mut F,
) -> Result<()>
where
    F: FnMut(AudioChunk),
{
    let mut decoded = ffmpeg::frame::Audio::empty();

    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut converted = ffmpeg::frame::Audio::empty();
        resampler
            .run(&decoded, &mut converted)
            .context("resample ffmpeg audio frame")?;
        emit_converted_audio(&converted, on_chunk);
    }

    Ok(())
}

fn flush_resampler<F>(
    resampler: &mut ffmpeg::software::resampling::Context,
    on_chunk: &mut F,
) -> Result<()>
where
    F: FnMut(AudioChunk),
{
    loop {
        let mut converted = ffmpeg::frame::Audio::empty();
        let delay = resampler.flush(&mut converted).context("flush ffmpeg audio resampler")?;
        if converted.samples() > 0 {
            emit_converted_audio(&converted, on_chunk);
        }
        if delay.is_none() || converted.samples() == 0 {
            break;
        }
    }

    Ok(())
}

fn emit_converted_audio<F>(frame: &ffmpeg::frame::Audio, on_chunk: &mut F)
where
    F: FnMut(AudioChunk),
{
    let channels = frame.channels().max(1);
    let samples = packed_f32_samples(frame, channels);
    if samples.is_empty() {
        return;
    }

    on_chunk(AudioChunk {
        channels,
        sample_rate: frame.rate().max(1),
        samples,
    });
}

fn packed_f32_samples(frame: &ffmpeg::frame::Audio, channels: u16) -> Vec<f32> {
    if frame.samples() == 0 || channels == 0 {
        return Vec::new();
    }

    let sample_count = frame.samples() * usize::from(channels);
    let bytes = frame.data(0);
    let available = bytes.len() / std::mem::size_of::<f32>();
    let sample_count = sample_count.min(available);
    if sample_count == 0 {
        return Vec::new();
    }

    let ptr = bytes.as_ptr() as *const f32;
    unsafe { std::slice::from_raw_parts(ptr, sample_count).to_vec() }
}
