use std::path::Path;
use std::time::Duration;

use app_media::AudioFileMetadata;
use app_mpv_ffi::probe_media as probe_mpv_media;

/// Estimated duration from container metadata (no full decode).
pub fn audio_file_duration(path: &Path) -> Option<Duration> {
    read_audio_metadata(path).and_then(|metadata| metadata.duration)
}

pub fn read_audio_metadata(path: &Path) -> Option<AudioFileMetadata> {
    probe_mpv_media(path).ok().map(|result| AudioFileMetadata {
        duration: result.duration,
        title: result.title,
        artist: result.artist,
        album: result.album,
        codec: result.audio_codec,
        sample_rate: result.sample_rate,
        channels: result.channels,
        bitrate_kbps: result.bitrate_kbps,
        file_size: result.file_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn audio_file_duration_none_for_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.mp3");
        std::fs::File::create(&path).unwrap();
        assert!(audio_file_duration(&path).is_none());
    }

    #[test]
    fn codec_label_uses_human_readable_names() {
        assert_eq!(codec_label(CODEC_TYPE_MP3), "MP3");
        assert_eq!(codec_label(CODEC_TYPE_FLAC), "FLAC");
    }
}
