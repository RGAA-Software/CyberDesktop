use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    File(PathBuf),
}

impl MediaSource {
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::File(path) => path,
        }
    }
}
