use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use tracing::warn;

use crate::{video_decode::{decode_video_frames, VideoFrame}, MediaSource};

#[derive(Debug)]
pub enum VideoDecodeEvent {
    Frame(VideoFrame),
    Finished,
    Error(String),
}

pub struct VideoDecodeHandle {
    cancel_decode: Arc<AtomicBool>,
    event_rx: Receiver<VideoDecodeEvent>,
}

impl VideoDecodeHandle {
    pub fn cancel(&self) {
        self.cancel_decode.store(true, Ordering::Relaxed);
    }

    pub fn try_recv(&self) -> Result<VideoDecodeEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }
}

pub fn spawn_video_decode(source: MediaSource) -> VideoDecodeHandle {
    let cancel_decode = Arc::new(AtomicBool::new(false));
    let (event_tx, event_rx) = mpsc::channel();
    let cancel_for_thread = Arc::clone(&cancel_decode);

    thread::Builder::new()
        .name("app-media-video-decode".into())
        .spawn(move || match source {
            MediaSource::File(path) => {
                let source = MediaSource::File(path);
                let result = decode_video_frames(&source, &cancel_for_thread, |frame| {
                    let _ = event_tx.send(VideoDecodeEvent::Frame(frame));
                });

                if cancel_for_thread.load(Ordering::Relaxed) {
                    let _ = event_tx.send(VideoDecodeEvent::Finished);
                    return;
                }

                match result {
                    Ok(()) => {
                        let _ = event_tx.send(VideoDecodeEvent::Finished);
                    }
                    Err(error) => {
                        warn!("app-media video decode failed: {error:#}");
                        let _ = event_tx.send(VideoDecodeEvent::Error(error.to_string()));
                    }
                }
            }
        })
        .expect("spawn app-media-video-decode thread");

    VideoDecodeHandle {
        cancel_decode,
        event_rx,
    }
}
