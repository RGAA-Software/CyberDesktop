use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use app_mpv_ffi::{probe_media as probe_mpv_media, MpvAudioPlayer};

pub struct AudioPlayer {
    cmd_tx: mpsc::Sender<AudioCommand>,
    state: Arc<Mutex<AudioState>>,
    _thread: thread::JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct AudioState {
    pub active_path: Option<PathBuf>,
    pub total: Option<Duration>,
    pub position: Duration,
    pub paused: bool,
    pub finished: bool,
    pub play_error: Option<String>,
}

enum AudioCommand {
    Play(PathBuf),
    TogglePause,
    SeekRelative(f64),
    SeekTo(Duration),
    Stop,
    SetVolume(f64),
    SetMute(bool),
    SetSpeed(f64),
}

struct PlaybackSession {
    player: MpvAudioPlayer,
    path: PathBuf,
    total: Option<Duration>,
    paused: bool,
}

impl AudioPlayer {
    pub fn start() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(AudioState {
            active_path: None,
            total: None,
            position: Duration::ZERO,
            paused: false,
            finished: false,
            play_error: None,
        }));
        let state_for_thread = Arc::clone(&state);
        let thread = thread::Builder::new()
            .name("media-player-audio".into())
            .spawn(move || audio_thread_main(cmd_rx, state_for_thread))
            .expect("spawn media-player-audio thread");
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
        if let Ok(mut guard) = self.state.lock() {
            guard.active_path = Some(path.clone());
            guard.total = None;
            guard.position = Duration::ZERO;
            guard.paused = false;
            guard.play_error = None;
            guard.finished = false;
        }
        let _ = self.cmd_tx.send(AudioCommand::Play(path));
    }

    pub fn toggle_pause(&self) {
        let _ = self.cmd_tx.send(AudioCommand::TogglePause);
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(AudioCommand::Stop);
    }

    pub fn seek_relative(&self, seconds: f64) {
        let _ = self.cmd_tx.send(AudioCommand::SeekRelative(seconds));
    }

    pub fn seek_to(&self, position: Duration) {
        let _ = self.cmd_tx.send(AudioCommand::SeekTo(position));
    }

    pub fn is_active_path(&self, path: &Path) -> bool {
        self.snapshot()
            .active_path
            .as_deref()
            .is_some_and(|active| active == path)
    }

    pub fn set_volume(&self, volume: f64) {
        let _ = self.cmd_tx.send(AudioCommand::SetVolume(volume));
    }

    pub fn set_mute(&self, mute: bool) {
        let _ = self.cmd_tx.send(AudioCommand::SetMute(mute));
    }

    pub fn set_speed(&self, speed: f64) {
        let _ = self.cmd_tx.send(AudioCommand::SetSpeed(speed));
    }
}

fn audio_thread_main(cmd_rx: mpsc::Receiver<AudioCommand>, state: Arc<Mutex<AudioState>>) {
    let mut session: Option<PlaybackSession> = None;
    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(AudioCommand::Play(path)) => {
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
                        Err(_) => {}
                    }
                }
            }
            Ok(AudioCommand::Stop) => {
                if let Some(mut session) = session.take() {
                    let _ = session.player.stop();
                }
                clear_playback_state(&state);
            }
            Ok(AudioCommand::SeekRelative(seconds)) => {
                if let Some(session) = session.as_mut() {
                    let _ = session.player.seek_relative(seconds);
                    let position = session.player.time_pos().unwrap_or(None);
                    sync_session_state(&state, session, position);
                }
            }
            Ok(AudioCommand::SeekTo(position)) => {
                if let Some(session) = session.as_mut() {
                    let _ = session.player.seek_to(position);
                    let position = session.player.time_pos().unwrap_or(Some(position));
                    sync_session_state(&state, session, position);
                }
            }
            Ok(AudioCommand::SetVolume(volume)) => {
                if let Some(session) = session.as_mut() {
                    let _ = session.player.set_volume(volume);
                }
            }
            Ok(AudioCommand::SetMute(mute)) => {
                if let Some(session) = session.as_mut() {
                    let _ = session.player.set_mute(mute);
                }
            }
            Ok(AudioCommand::SetSpeed(speed)) => {
                if let Some(session) = session.as_mut() {
                    let _ = session.player.set_speed(speed);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut finished_path = None;
                if let Some(active) = session.as_mut() {
                    active.player.poll_events();
                    if active.player.ended() {
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
    }
}

fn clear_playback_state(state: &Arc<Mutex<AudioState>>) {
    if let Ok(mut guard) = state.lock() {
        guard.active_path = None;
        guard.total = None;
        guard.position = Duration::ZERO;
        guard.paused = false;
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
    }
}
