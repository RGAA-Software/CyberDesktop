use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

use tracing::warn;

use crate::{decode_audio_file, AudioChunk, MediaSource};

#[derive(Debug)]
pub enum AudioDecodeEvent {
    Chunk(AudioChunk),
    Finished,
    Error(String),
}

pub struct AudioDecodeHandle {
    cancel_decode: Arc<AtomicBool>,
    event_rx: Receiver<AudioDecodeEvent>,
}

impl AudioDecodeHandle {
    pub fn cancel(&self) {
        self.cancel_decode.store(true, Ordering::Relaxed);
    }

    pub fn try_recv(&self) -> Result<AudioDecodeEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }
}

pub fn spawn_audio_decode(source: MediaSource) -> AudioDecodeHandle {
    let cancel_decode = Arc::new(AtomicBool::new(false));
    let (event_tx, event_rx) = mpsc::channel();
    let cancel_for_thread = Arc::clone(&cancel_decode);

    thread::Builder::new()
        .name("app-media-audio-decode".into())
        .spawn(move || match source {
            MediaSource::File(path) => {
                let result = decode_audio_file(&path, &cancel_for_thread, |chunk| {
                    let _ = event_tx.send(AudioDecodeEvent::Chunk(chunk));
                });

                if cancel_for_thread.load(Ordering::Relaxed) {
                    let _ = event_tx.send(AudioDecodeEvent::Finished);
                    return;
                }

                match result {
                    Ok(()) => {
                        let _ = event_tx.send(AudioDecodeEvent::Finished);
                    }
                    Err(error) => {
                        warn!("app-media audio decode failed: {error:#}");
                        let _ = event_tx.send(AudioDecodeEvent::Error(error.to_string()));
                    }
                }
            }
        })
        .expect("spawn app-media-audio-decode thread");

    AudioDecodeHandle {
        cancel_decode,
        event_rx,
    }
}
