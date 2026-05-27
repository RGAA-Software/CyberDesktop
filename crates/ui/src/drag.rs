//! In-app file drag/drop payload shared by file list, omnibar, and sidebar.

use std::path::PathBuf;

/// Paths being dragged between CyberFiles drop targets (not OS shell drag type).
#[derive(Clone, Debug, Default)]
pub struct DraggedFilePaths(pub Vec<PathBuf>);
