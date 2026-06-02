use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use tracing::{debug, warn};

use crate::{MediaCommand, MediaError, MediaEvent, MediaFileMetadata, MediaSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSessionState {
    Idle,
    Probing,
    Ready,
    Playing,
    Paused,
    Seeking,
    Ended,
    Failed,
}

pub struct MediaController {
    cmd_tx: Sender<MediaCommand>,
}

impl MediaController {
    pub fn new() -> (Self, Receiver<MediaEvent>) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        thread::Builder::new()
            .name("app-media-session".into())
            .spawn(move || MediaSession::default().run(cmd_rx, event_tx))
            .expect("spawn app-media-session thread");

        (Self { cmd_tx }, event_rx)
    }

    pub fn open(&self, source: MediaSource) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Open(source))
    }

    pub fn play(&self) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Play)
    }

    pub fn pause(&self) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Pause)
    }

    pub fn stop(&self) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Stop)
    }

    pub fn seek(&self, position: Duration) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Seek(position))
    }

    pub fn close(&self) -> Result<(), mpsc::SendError<MediaCommand>> {
        self.cmd_tx.send(MediaCommand::Close)
    }
}

#[derive(Default)]
struct MediaSession {
    current_source: Option<MediaSource>,
    current_metadata: Option<MediaFileMetadata>,
    state: Option<MediaSessionState>,
}

impl MediaSession {
    fn run(mut self, cmd_rx: Receiver<MediaCommand>, event_tx: Sender<MediaEvent>) {
        self.set_state(MediaSessionState::Idle, &event_tx);

        while let Ok(command) = cmd_rx.recv() {
            debug!("app-media session command: {:?}", command);
            let keep_running = self.handle_command(command, &event_tx);
            if !keep_running {
                break;
            }
        }
    }

    fn handle_command(&mut self, command: MediaCommand, event_tx: &Sender<MediaEvent>) -> bool {
        match command {
            MediaCommand::Open(source) => self.open(source, event_tx),
            MediaCommand::Play => self.play(event_tx),
            MediaCommand::Pause => self.pause(event_tx),
            MediaCommand::Stop => self.stop(event_tx),
            MediaCommand::Seek(position) => self.seek(position, event_tx),
            MediaCommand::Close => {
                self.current_source = None;
                self.current_metadata = None;
                self.set_state(MediaSessionState::Idle, event_tx);
                false
            }
        }
    }

    fn open(&mut self, source: MediaSource, event_tx: &Sender<MediaEvent>) -> bool {
        self.current_source = Some(source.clone());
        self.current_metadata = None;
        self.set_state(MediaSessionState::Probing, event_tx);

        match crate::probe_media(&source) {
            Ok(result) => {
                self.current_metadata = Some(result.metadata.clone());
                let _ = event_tx.send(MediaEvent::Probed(result.metadata));
                self.set_state(MediaSessionState::Ready, event_tx);
            }
            Err(error) => {
                self.current_source = None;
                self.current_metadata = None;
                self.send_error(
                    MediaError::ProbeFailed(format!("failed to probe media: {error:#}")),
                    event_tx,
                );
                self.set_state(MediaSessionState::Failed, event_tx);
            }
        }

        true
    }

    fn play(&mut self, event_tx: &Sender<MediaEvent>) -> bool {
        if self.current_source.is_none() || self.current_metadata.is_none() {
            self.send_error(
                MediaError::OpenFailed("cannot play without an opened media source".into()),
                event_tx,
            );
            return true;
        }

        self.set_state(MediaSessionState::Playing, event_tx);
        true
    }

    fn pause(&mut self, event_tx: &Sender<MediaEvent>) -> bool {
        if !matches!(self.state, Some(MediaSessionState::Playing)) {
            self.send_error(
                MediaError::Unsupported("pause is only valid while playback is active".into()),
                event_tx,
            );
            return true;
        }

        self.set_state(MediaSessionState::Paused, event_tx);
        true
    }

    fn stop(&mut self, event_tx: &Sender<MediaEvent>) -> bool {
        if self.current_source.is_none() {
            return true;
        }

        self.set_state(MediaSessionState::Ended, event_tx);
        self.set_state(MediaSessionState::Ready, event_tx);
        let _ = event_tx.send(MediaEvent::PositionUpdated(Duration::ZERO));
        true
    }

    fn seek(&mut self, position: Duration, event_tx: &Sender<MediaEvent>) -> bool {
        if self.current_source.is_none() || self.current_metadata.is_none() {
            self.send_error(
                MediaError::SeekFailed("cannot seek without an opened media source".into()),
                event_tx,
            );
            return true;
        }

        let resume_state = match self.state {
            Some(MediaSessionState::Playing) => MediaSessionState::Playing,
            Some(MediaSessionState::Paused) => MediaSessionState::Paused,
            _ => MediaSessionState::Ready,
        };
        self.set_state(MediaSessionState::Seeking, event_tx);
        let _ = event_tx.send(MediaEvent::PositionUpdated(position));
        self.set_state(resume_state, event_tx);
        true
    }

    fn set_state(&mut self, next_state: MediaSessionState, event_tx: &Sender<MediaEvent>) {
        if self.state.as_ref() == Some(&next_state) {
            return;
        }

        self.state = Some(next_state.clone());
        let _ = event_tx.send(MediaEvent::StateChanged(next_state));
    }

    fn send_error(&self, error: MediaError, event_tx: &Sender<MediaEvent>) {
        warn!("app-media session error: {error}");
        let _ = event_tx.send(MediaEvent::Error(error));
    }
}
