use std::sync::OnceLock;

use anyhow::Result;

pub(crate) fn init_ffmpeg() -> Result<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();

    INIT.get_or_init(|| ffmpeg_next::init().map_err(|err| err.to_string()))
        .as_ref()
        .map_err(|err| anyhow::anyhow!(err.clone()))?;

    Ok(())
}
