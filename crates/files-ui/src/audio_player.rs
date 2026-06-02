use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use app_media::{
    spawn_audio_decode, AudioDecodeEvent, AudioDecodeHandle, MediaController, MediaEvent,
    MediaSessionState, MediaSource,
};
use anyhow::Context as _;
use rodio::{buffer::SamplesBuffer, OutputStream, OutputStreamHandle, Sink};

use crate::audio_log::audio_log;

/// Non-blocking handle; `OutputStream` + playback live on `info-pane-audio`.
pub struct AudioPlayer {
    cmd_tx: Sender<AudioCommand>,
    state: Arc<Mutex<AudioState>>,
    _thread: JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct AudioState {
    pub output_ok: bool,
    pub active_path: Option<PathBuf>,
    pub total: Option<Duration>,
    pub position: Duration,
    pub paused: bool,
    pub media_state: MediaSessionState,
    pub play_error: Option<String>,
    pub finished: bool,
}

enum AudioCommand {
    Play(PathBuf),
    TogglePause,
    Stop,
}

struct PlaybackSession {
    sink: Arc<Sink>,
    path: PathBuf,
    total: Option<Duration>,
    started_at: Instant,
    paused_at: Option<Instant>,
    paused_total: Duration,
    decode_done: bool,
    decoder: AudioDecodeHandle,
}

impl PlaybackSession {
    fn position(&self) -> Duration {
        let elapsed = match self.paused_at {
            Some(paused_at) => paused_at.saturating_duration_since(self.started_at),
            None => Instant::now().saturating_duration_since(self.started_at),
        };
        let position = elapsed.saturating_sub(self.paused_total);
        self.total.map_or(position, |total| position.min(total))
    }

    fn toggle_pause(&mut self) {
        if self.sink.is_paused() {
            if let Some(paused_at) = self.paused_at.take() {
                self.paused_total += Instant::now().saturating_duration_since(paused_at);
            }
            self.sink.play();
        } else {
            self.paused_at = Some(Instant::now());
            self.sink.pause();
        }
    }

    fn finish_decode(&self) -> bool {
        self.decode_done
    }

    fn cancel_decode(&self) {
        self.decoder.cancel();
    }
}

impl AudioPlayer {
    pub fn start() -> Self {
        audio_log!("AudioPlayer::start spawning thread");
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(AudioState {
            output_ok: false,
            active_path: None,
            total: None,
            position: Duration::ZERO,
            paused: false,
            media_state: MediaSessionState::Idle,
            play_error: None,
            finished: false,
        }));
        let state_for_thread = Arc::clone(&state);
        let thread = thread::Builder::new()
            .name("info-pane-audio".into())
            .spawn(move || audio_thread_main(cmd_rx, state_for_thread))
            .expect("spawn info-pane-audio thread");
        audio_log!("AudioPlayer::start thread spawned");
        Self {
            cmd_tx,
            state,
            _thread: thread,
        }
    }

    pub fn snapshot(&self) -> AudioState {
        self.state
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }

    pub fn play(&self, path: PathBuf) {
        audio_log!("play command send {}", path.display());
        if let Ok(mut guard) = self.state.lock() {
            // Mark the requested path immediately so UI selection refreshes
            // don't clear playback before the audio thread consumes Play.
            guard.active_path = Some(path.clone());
            guard.total = None;
            guard.position = Duration::ZERO;
            guard.paused = false;
            guard.play_error = None;
            guard.finished = false;
        }
        self.send(AudioCommand::Play(path));
    }

    pub fn toggle_pause(&self) {
        self.send(AudioCommand::TogglePause);
    }

    pub fn stop(&self) {
        audio_log!("stop command send");
        self.send(AudioCommand::Stop);
    }

    pub fn is_active_path(&self, path: &Path) -> bool {
        self.snapshot()
            .active_path
            .as_deref()
            .is_some_and(|active| active == path)
    }

    pub fn is_paused(&self) -> bool {
        self.snapshot().paused
    }

    pub fn position(&self, path: &Path) -> Option<Duration> {
        let state = self.snapshot();
        if state.active_path.as_deref() == Some(path) {
            Some(state.position)
        } else {
            None
        }
    }

    pub fn total_duration(&self, path: &Path) -> Option<Duration> {
        let state = self.snapshot();
        if state.active_path.as_deref() == Some(path) {
            state.total
        } else {
            None
        }
    }

    pub fn take_finished(&self, path: &Path) -> bool {
        let mut guard = self.state.lock().expect("audio state");
        if guard.active_path.as_deref() == Some(path) && guard.finished {
            guard.finished = false;
            true
        } else {
            false
        }
    }

    fn send(&self, cmd: AudioCommand) {
        if self.cmd_tx.send(cmd).is_err() {
            audio_log!("command send failed (audio thread gone)");
        }
    }
}

fn audio_thread_main(cmd_rx: Receiver<AudioCommand>, state: Arc<Mutex<AudioState>>) {
    let (media_controller, media_events) = MediaController::new();
    audio_log!("audio thread: OutputStream::try_default");
    let t0 = Instant::now();
    let output = OutputStream::try_default();
    audio_log!(
        "audio thread: OutputStream done in {:?} err={}",
        t0.elapsed(),
        output.is_err()
    );

    let Ok((stream, handle)) = output else {
        let error = output.err().map(|e| e.to_string()).unwrap_or_default();
        audio_log!("audio thread: no output: {error}");
        if let Ok(mut guard) = state.lock() {
            guard.output_ok = false;
            guard.media_state = MediaSessionState::Failed;
            guard.play_error = Some(if error.is_empty() {
                "no audio output device".into()
            } else {
                error
            });
        }
        drain_commands_failed(cmd_rx, &state);
        return;
    };
    let _stream = stream;

    if let Ok(mut guard) = state.lock() {
        guard.output_ok = true;
        guard.media_state = MediaSessionState::Idle;
        guard.play_error = None;
    }
    audio_log!("audio thread: ready");

    let mut session: Option<PlaybackSession> = None;

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(AudioCommand::Play(path)) => {
                audio_log!("audio thread: Play {}", path.display());
                if let Ok(mut guard) = state.lock() {
                    guard.media_state = MediaSessionState::Probing;
                    guard.play_error = None;
                    guard.finished = false;
                }
                let _ = media_controller.open(MediaSource::File(path.clone()));
                let _ = media_controller.play();
                session = start_playback(&handle, &path, &state);
                if let Some(session) = session.as_ref() {
                    audio_log!(
                        "audio thread: playing empty={} paused={} pos={:?}",
                        session.sink.empty(),
                        session.sink.is_paused(),
                        session.sink.get_pos()
                    );
                }
            }
            Ok(AudioCommand::TogglePause) => {
                if let Some(session) = session.as_mut() {
                    session.toggle_pause();
                    if session.sink.is_paused() {
                        let _ = media_controller.pause();
                    } else {
                        let _ = media_controller.play();
                    }
                    sync_session_state(&state, session);
                }
            }
            Ok(AudioCommand::Stop) => {
                if let Some(session) = session.take() {
                    session.cancel_decode();
                }
                let _ = media_controller.stop();
                session = None;
                clear_playback_state(&state);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(session) = session.as_mut() {
                    drain_audio_decode_events(session, &state);
                }
                let track_finished = session.as_ref().is_some_and(|session| {
                    session.sink.empty()
                        && session.finish_decode()
                        && session.started_at.elapsed() > Duration::from_millis(400)
                        && session.position() > Duration::ZERO
                });
                if track_finished {
                    audio_log!("audio thread: track finished");
                    let _ = media_controller.stop();
                    session = None;
                    if let Ok(mut guard) = state.lock() {
                        guard.finished = true;
                        guard.active_path = None;
                        guard.position = Duration::ZERO;
                        guard.paused = false;
                        guard.media_state = MediaSessionState::Ended;
                    }
                } else if let Some(session) = session.as_ref() {
                    sync_session_state(&state, session);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        drain_media_events(&media_events, &state);
    }
}

fn drain_commands_failed(cmd_rx: Receiver<AudioCommand>, state: &Arc<Mutex<AudioState>>) {
    while let Ok(cmd) = cmd_rx.recv() {
        if matches!(cmd, AudioCommand::Play(_)) {
            if let Ok(mut guard) = state.lock() {
                if guard.play_error.is_none() {
                    guard.play_error = Some("no audio output device".into());
                }
            }
        }
    }
}

fn start_playback(
    handle: &OutputStreamHandle,
    path: &Path,
    state: &Arc<Mutex<AudioState>>,
) -> Option<PlaybackSession> {
    let result = (|| {
        let sink = Arc::new(Sink::try_new(handle).context("open audio output")?);
        sink.set_volume(1.0);
        sink.play();
        let total = files_fs::audio_file_duration(path);
        let decoder = spawn_audio_decode(MediaSource::File(path.to_path_buf()));
        Ok::<_, anyhow::Error>(PlaybackSession {
            sink,
            path: path.to_path_buf(),
            total,
            started_at: Instant::now(),
            paused_at: None,
            paused_total: Duration::ZERO,
            decode_done: false,
            decoder,
        })
    })();

    match result {
        Ok(session) => {
            if let Ok(mut guard) = state.lock() {
                guard.play_error = None;
                guard.finished = false;
                guard.active_path = Some(session.path.clone());
                guard.total = session.total;
                guard.position = Duration::ZERO;
                guard.paused = false;
            }
            Some(session)
        }
        Err(error) => {
            audio_log!("start_playback: err {error:#}");
            if let Ok(mut guard) = state.lock() {
                guard.play_error = Some(error.to_string());
                guard.active_path = None;
                guard.total = None;
                guard.position = Duration::ZERO;
                guard.paused = false;
                guard.finished = false;
            }
            None
        }
    }
}

fn sync_session_state(state: &Arc<Mutex<AudioState>>, session: &PlaybackSession) {
    if let Ok(mut guard) = state.lock() {
        guard.active_path = Some(session.path.clone());
        guard.total = session.total;
        guard.position = session.position();
        guard.paused = session.sink.is_paused();
        guard.play_error = None;
        guard.finished = false;
        if session.sink.is_paused() {
            guard.media_state = MediaSessionState::Paused;
        } else {
            guard.media_state = MediaSessionState::Playing;
        }
    }
}

fn clear_playback_state(state: &Arc<Mutex<AudioState>>) {
    if let Ok(mut guard) = state.lock() {
        guard.active_path = None;
        guard.total = None;
        guard.position = Duration::ZERO;
        guard.paused = false;
        guard.media_state = MediaSessionState::Ready;
        guard.finished = false;
        guard.play_error = None;
    }
}

fn drain_media_events(event_rx: &Receiver<MediaEvent>, state: &Arc<Mutex<AudioState>>) {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            MediaEvent::Probed(metadata) => {
                if let Ok(mut guard) = state.lock() {
                    if metadata.source_path.as_ref() == guard.active_path.as_ref() {
                        guard.total = metadata
                            .audio
                            .as_ref()
                            .and_then(|audio| audio.duration)
                            .or(metadata.duration)
                            .or(guard.total);
                    }
                }
            }
            MediaEvent::StateChanged(media_state) => {
                if let Ok(mut guard) = state.lock() {
                    guard.media_state = media_state;
                }
            }
            MediaEvent::PositionUpdated(position) => {
                if let Ok(mut guard) = state.lock() {
                    if matches!(guard.media_state, MediaSessionState::Seeking | MediaSessionState::Ended) {
                        guard.position = position;
                    }
                }
            }
            MediaEvent::Error(error) => {
                audio_log!("media session event error: {error}");
            }
        }
    }
}

fn drain_audio_decode_events(session: &mut PlaybackSession, state: &Arc<Mutex<AudioState>>) {
    loop {
        match session.decoder.try_recv() {
            Ok(AudioDecodeEvent::Chunk(chunk)) => {
                session.total = session.total.or_else(|| {
                    if chunk.sample_rate == 0 || chunk.channels == 0 {
                        None
                    } else {
                        None
                    }
                });
                append_chunk_samples(
                    &chunk.samples,
                    chunk.channels,
                    chunk.sample_rate,
                    &session.sink,
                );
            }
            Ok(AudioDecodeEvent::Finished) => {
                session.decode_done = true;
                break;
            }
            Ok(AudioDecodeEvent::Error(error)) => {
                audio_log!("decode thread: err {error}");
                session.decode_done = true;
                if let Ok(mut guard) = state.lock() {
                    if guard.active_path.as_deref() == Some(session.path.as_path()) {
                        guard.play_error = Some(error);
                    }
                }
                break;
            }
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                session.decode_done = true;
                break;
            }
        }
    }
}

fn append_chunk_samples(
    frame_samples: &[f32],
    out_channels: u16,
    out_rate: u32,
    sink: &Sink,
) {
    if frame_samples.is_empty() {
        return;
    }

    sink.append(SamplesBuffer::new(
        out_channels.max(1),
        out_rate.max(1),
        frame_samples.to_vec(),
    ));
}
