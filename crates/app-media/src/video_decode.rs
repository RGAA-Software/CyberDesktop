use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use ffmpeg_next as ffmpeg;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};

use crate::{ffmpeg_util::init_ffmpeg, MediaSource};

#[derive(Debug, Clone)]
pub struct VideoPoster {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub bytes_per_row: u32,
    pub bgra_bytes: Vec<u8>,
    pub position: Duration,
}

pub fn extract_video_poster(source: &MediaSource) -> Result<VideoPoster> {
    init_ffmpeg()?;

    let path = source.path();
    let mut input = ffmpeg::format::input(path)
        .with_context(|| format!("open {}", path.display()))?;
    let video_stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| anyhow::anyhow!("no default video track"))?;
    let stream_index = video_stream.index();
    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .context("create ffmpeg video codec context")?;
    let mut decoder = codec_context
        .decoder()
        .video()
        .context("open ffmpeg video decoder")?;

    let width = decoder.width();
    let height = decoder.height();
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::BGRA,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .context("create ffmpeg video scaler")?;

    for (stream, packet) in input.packets() {
        if stream.index() != stream_index {
            continue;
        }

        decoder
            .send_packet(&packet)
            .context("send ffmpeg video packet")?;
        if let Some(poster) = receive_first_video_frame(&mut decoder, &mut scaler)? {
            return Ok(poster);
        }
    }

    decoder.send_eof().context("send ffmpeg video eof")?;
    if let Some(poster) = receive_first_video_frame(&mut decoder, &mut scaler)? {
        return Ok(poster);
    }

    bail!("no decodable video frame found")
}

fn receive_first_video_frame(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ffmpeg::software::scaling::context::Context,
) -> Result<Option<VideoPoster>> {
    let mut decoded = ffmpeg::frame::Video::empty();

    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut rgb = ffmpeg::frame::Video::empty();
        scaler
            .run(&decoded, &mut rgb)
            .context("scale ffmpeg video frame")?;
        return Ok(Some(video_frame_to_poster(&rgb)?));
    }

    Ok(None)
}

pub fn decode_video_frames<F>(
    source: &MediaSource,
    cancel_decode: &AtomicBool,
    mut on_frame: F,
) -> Result<()>
where
    F: FnMut(VideoFrame),
{
    init_ffmpeg()?;

    let path = source.path();
    let mut input = ffmpeg::format::input(path)
        .with_context(|| format!("open {}", path.display()))?;
    let video_stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| anyhow::anyhow!("no default video track"))?;
    let stream_index = video_stream.index();
    let time_base = video_stream.time_base();
    let frame_interval = frame_interval(video_stream.avg_frame_rate());
    let codec_context = ffmpeg::codec::context::Context::from_parameters(video_stream.parameters())
        .context("create ffmpeg video codec context")?;
    let mut decoder = codec_context
        .decoder()
        .video()
        .context("open ffmpeg video decoder")?;

    let width = decoder.width();
    let height = decoder.height();
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::BGRA,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .context("create ffmpeg video scaler")?;

    let playback_started_at = Instant::now();
    let mut first_position = None;
    let mut last_position = None;

    for (stream, packet) in input.packets() {
        if cancel_decode.load(Ordering::Relaxed) {
            return Ok(());
        }
        if stream.index() != stream_index {
            continue;
        }

        decoder
            .send_packet(&packet)
            .context("send ffmpeg video packet")?;
        receive_decoded_video_frames(
            &mut decoder,
            &mut scaler,
            time_base,
            frame_interval,
            &mut first_position,
            &mut last_position,
            playback_started_at,
            cancel_decode,
            &mut on_frame,
        )?;
    }

    decoder.send_eof().context("send ffmpeg video eof")?;
    receive_decoded_video_frames(
        &mut decoder,
        &mut scaler,
        time_base,
        frame_interval,
        &mut first_position,
        &mut last_position,
        playback_started_at,
        cancel_decode,
        &mut on_frame,
    )?;

    Ok(())
}

fn receive_decoded_video_frames<F>(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ffmpeg::software::scaling::context::Context,
    time_base: ffmpeg::Rational,
    frame_interval: Option<Duration>,
    first_position: &mut Option<Duration>,
    last_position: &mut Option<Duration>,
    playback_started_at: Instant,
    cancel_decode: &AtomicBool,
    on_frame: &mut F,
) -> Result<()>
where
    F: FnMut(VideoFrame),
{
    let mut decoded = ffmpeg::frame::Video::empty();

    while decoder.receive_frame(&mut decoded).is_ok() {
        if cancel_decode.load(Ordering::Relaxed) {
            return Ok(());
        }

        let relative_position = resolve_frame_position(
            frame_position(&decoded, time_base),
            frame_interval,
            first_position,
            last_position,
        );
        sync_video_playback(playback_started_at, relative_position, cancel_decode);

        let mut rgb = ffmpeg::frame::Video::empty();
        scaler
            .run(&decoded, &mut rgb)
            .context("scale ffmpeg video frame")?;
        let frame = video_frame_to_bgra(&rgb)?;
        on_frame(VideoFrame {
            width: frame.width,
            height: frame.height,
            bytes_per_row: frame.bytes_per_row,
            bgra_bytes: frame.bgra_bytes,
            position: relative_position,
        });
    }

    Ok(())
}

fn resolve_frame_position(
    raw_position: Option<Duration>,
    frame_interval: Option<Duration>,
    first_position: &mut Option<Duration>,
    last_position: &mut Option<Duration>,
) -> Duration {
    let position = if let Some(raw_position) = raw_position {
        let base_position = *first_position.get_or_insert(raw_position);
        let relative = raw_position.saturating_sub(base_position);
        match *last_position {
            Some(last) if relative <= last => {
                if let Some(interval) = frame_interval {
                    last.saturating_add(interval)
                } else {
                    relative
                }
            }
            _ => relative,
        }
    } else if let Some(last) = *last_position {
        if let Some(interval) = frame_interval {
            last.saturating_add(interval)
        } else {
            last
        }
    } else {
        Duration::ZERO
    };

    *last_position = Some(position);
    position
}

fn frame_position(frame: &ffmpeg::frame::Video, time_base: ffmpeg::Rational) -> Option<Duration> {
    let timestamp = frame.timestamp().or_else(|| frame.pts())?;
    if timestamp < 0 {
        return None;
    }

    let numerator = f64::from(time_base.numerator());
    let denominator = f64::from(time_base.denominator());
    if denominator <= 0.0 {
        return None;
    }
    let secs = timestamp as f64 * numerator / denominator;
    if secs.is_finite() && secs >= 0.0 {
        Some(Duration::from_secs_f64(secs))
    } else {
        None
    }
}

fn frame_interval(rate: ffmpeg::Rational) -> Option<Duration> {
    let denominator = f64::from(rate.denominator());
    if denominator <= 0.0 {
        return None;
    }

    let fps = f64::from(rate.numerator()) / denominator;
    if fps.is_finite() && fps > 0.0 {
        Some(Duration::from_secs_f64(1.0 / fps))
    } else {
        None
    }
}

fn sync_video_playback(
    playback_started_at: Instant,
    frame_position: Duration,
    cancel_decode: &AtomicBool,
) {
    loop {
        if cancel_decode.load(Ordering::Relaxed) {
            return;
        }
        let elapsed = playback_started_at.elapsed();
        if elapsed >= frame_position {
            return;
        }
        let remaining = frame_position.saturating_sub(elapsed);
        let sleep_for = remaining.min(Duration::from_millis(5));
        thread::sleep(sleep_for);
    }
}

struct BgraVideoFrame {
    width: u32,
    height: u32,
    bytes_per_row: u32,
    bgra_bytes: Vec<u8>,
}

fn video_frame_to_bgra(frame: &ffmpeg::frame::Video) -> Result<BgraVideoFrame> {
    let width = frame.width();
    let height = frame.height();
    if width == 0 || height == 0 {
        bail!("video frame has zero size");
    }

    let row_len = width as usize * 4;
    let stride = frame.stride(0);
    let data = frame.data(0);
    let mut bgra_bytes = Vec::with_capacity(row_len * height as usize);
    for row in 0..height as usize {
        let start = row
            .checked_mul(stride)
            .context("video frame stride overflow")?;
        let end = start
            .checked_add(row_len)
            .context("video frame row overflow")?;
        if end > data.len() {
            bail!("video frame buffer too small");
        }
        bgra_bytes.extend_from_slice(&data[start..end]);
    }

    Ok(BgraVideoFrame {
        width,
        height,
        bytes_per_row: row_len as u32,
        bgra_bytes,
    })
}

fn video_frame_to_poster(frame: &ffmpeg::frame::Video) -> Result<VideoPoster> {
    let bgra = video_frame_to_bgra(frame)?;
    let mut rgb_bytes = bgra.bgra_bytes;
    for pixel in rgb_bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let mut png_bytes = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(&rgb_bytes, bgra.width, bgra.height, ColorType::Rgba8.into())
        .context("encode video poster png")?;

    Ok(VideoPoster {
        png_bytes,
        width: bgra.width,
        height: bgra.height,
    })
}
