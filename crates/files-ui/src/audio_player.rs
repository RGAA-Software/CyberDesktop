use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use app_media::MediaSessionState;
use app_mpv_ffi::{probe_media as probe_mpv_media, MpvAudioPlayer};

use crate::audio_log::audio_log;

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
    player: MpvAudioPlayer,
    path: PathBuf,
    total: Option<Duration>,
    paused: bool,
}

impl AudioPlayer {
    pub fn start() -> Self {
        audio_log!("AudioPlayer::start spawning thread");
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(AudioState {
            output_ok: true,
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
            guard.active_path = Some(path.clone());
            guard.total = None;
            guard.position = Duration::ZERO;
            guard.paused = false;
            guard.play_error = None;
            guard.finished = false;
            guard.media_state = MediaSessionState::Probing;
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
             guard.active_path = None;
             guard.total = None;
             guard.position = Duration::ZERO;
             guard.paused = false;
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
    let mut session: Option<PlaybackSession> = None;

    if let Ok(mut guard) = state.lock() {
        guard.output_ok = true;
        guard.media_state = MediaSessionState::Idle;
        guard.play_error = None;
    }

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(AudioCommand::Play(path)) => {
                audio_log!("audio thread: Play {}", path.display());
                if let Some(mut old) = session.take() {
                    let _ = old.player.stop();
                }
                session = start_playback(&path, &state);
            }
            Ok(AudioCommand::TogglePause) => {
                if let Some(session) = session.as_mut() {
                    let paused = !session.paused;
                    match session.player.set_pause(paused) {
                        Ok(()) => {
                            session.paused = paused;
                            sync_session_state(&state, session, None);
                        }
                        Err(error) => {
                            set_play_error(&state, &session.path, error.to_string());
                        }
                    }
                }
            }
            Ok(AudioCommand::Stop) => {
                if let Some(mut session) = session.take() {
                    let _ = session.player.stop();
                }
                clear_playback_state(&state);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut finished_path = None;
                if let Some(active) = session.as_mut() {
                    active.player.poll_events();
                    if active.player.ended() {
                        audio_log!("audio thread: track finished");
                        finished_path = Some(active.path.clone());
                    } else {
                        let position = active.player.time_pos().unwrap_or(None);
                        let total = active.player.duration().unwrap_or(None).or(active.total);
                        active.total = total;
                        sync_session_state(&state, active, position);
                    }
                }
                if let Some(path) = finished_path {
                    session = None;
                    mark_finished(&state, path);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn start_playback(path: &Path, state: &Arc<Mutex<AudioState>>) -> Option<PlaybackSession> {
    let result = (|| {
        let metadata = probe_mpv_media(path).ok();
        let mut player = MpvAudioPlayer::new()?;
        player.load_file(path)?;
        Ok::<_, anyhow::Error>(PlaybackSession {
            player,
            path: path.to_path_buf(),
            total: metadata.as_ref().and_then(|info| info.duration),
            paused: false,
        })
    })();

    match result {
        Ok(session) => {
            if let Ok(mut guard) = state.lock() {
                guard.output_ok = true;
                guard.play_error = None;
                guard.finished = false;
                guard.active_path = Some(session.path.clone());
                guard.total = session.total;
                guard.position = Duration::ZERO;
                guard.paused = false;
                guard.media_state = MediaSessionState::Playing;
            }
            Some(session)
        }
        Err(error) => {
            audio_log!("start_playback: err {error:#}");
            if let Ok(mut guard) = state.lock() {
                guard.output_ok = false;
                guard.play_error = Some(error.to_string());
                guard.active_path = None;
                guard.total = None;
                guard.position = Duration::ZERO;
                guard.paused = false;
                guard.finished = false;
                guard.media_state = MediaSessionState::Failed;
            }
            None
        }
    }
}

fn sync_session_state(
    state: &Arc<Mutex<AudioState>>,
    session: &PlaybackSession,
    position: Option<Duration>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.active_path = Some(session.path.clone());
        guard.total = session.total;
        guard.position = position.unwrap_or(guard.position);
        guard.paused = session.paused;
        guard.play_error = None;
        guard.finished = false;
        guard.output_ok = true;
        guard.media_state = if session.paused {
            MediaSessionState::Paused
        } else {
            MediaSessionState::Playing
        };
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

fn mark_finished(state: &Arc<Mutex<AudioState>>, path: PathBuf) {
    if let Ok(mut guard) = state.lock() {
        guard.finished = true;
        guard.active_path = Some(path);
        guard.position = Duration::ZERO;
        guard.paused = false;
        guard.media_state = MediaSessionState::Ended;
    }
}

fn set_play_error(state: &Arc<Mutex<AudioState>>, path: &Path, error: String) {
    if let Ok(mut guard) = state.lock() {
        if guard.active_path.as_deref() == Some(path) {
            guard.play_error = Some(error);
            guard.media_state = MediaSessionState::Failed;
        }
    }
}
