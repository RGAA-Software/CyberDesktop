use std::time::Duration;

use crate::source::MediaSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaCommand {
    Open(MediaSource),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    Close,
}
