use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaError {
    Unsupported(String),
    OpenFailed(String),
    ProbeFailed(String),
    DecoderInitFailed(String),
    HardwareDecodeFailed(String),
    AudioOutputFailed(String),
    VideoRenderFailed(String),
    SeekFailed(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message)
            | Self::OpenFailed(message)
            | Self::ProbeFailed(message)
            | Self::DecoderInitFailed(message)
            | Self::HardwareDecodeFailed(message)
            | Self::AudioOutputFailed(message)
            | Self::VideoRenderFailed(message)
            | Self::SeekFailed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for MediaError {}
