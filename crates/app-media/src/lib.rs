mod audio_decode;
mod audio_stream;
mod command;
mod error;
mod event;
mod ffmpeg_util;
mod metadata;
mod probe;
mod session;
mod source;
mod video_decode;
mod video_stream;

pub use audio_decode::{decode_audio_file, AudioChunk};
pub use audio_stream::{spawn_audio_decode, AudioDecodeEvent, AudioDecodeHandle};
pub use command::MediaCommand;
pub use error::MediaError;
pub use event::MediaEvent;
pub use metadata::{
    AudioFileMetadata, MediaFileMetadata, MediaKind, MediaPlaybackClock, VideoDecodeMode,
    VideoFileMetadata,
};
pub use probe::{probe_media, MediaProbeResult};
pub use session::{MediaController, MediaSessionState};
pub use source::MediaSource;
pub use video_decode::{extract_video_poster, VideoFrame, VideoPoster};
pub use video_stream::{spawn_video_decode, VideoDecodeEvent, VideoDecodeHandle};
