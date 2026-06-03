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
