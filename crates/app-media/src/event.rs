use std::time::Duration;

use crate::{
    error::MediaError,
    metadata::MediaFileMetadata,
    session::MediaSessionState,
};

#[derive(Debug, Clone)]
pub enum MediaEvent {
    Probed(MediaFileMetadata),
    StateChanged(MediaSessionState),
    PositionUpdated(Duration),
    Error(MediaError),
}
