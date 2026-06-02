use std::path::{Path, PathBuf};

use crate::clipboard::{transfer_one, ClipboardOperation};
use crate::ops::{create_directory, create_file, recycle_paths, rename_path};
use crate::recycle::restore_recycled_originals;

const MAX_HISTORY: usize = 50;

/// A reversible file operation recorded for undo/redo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOperation {
    Move {
        moves: Vec<(PathBuf, PathBuf)>,
    },
    Copy {
        copies: Vec<(PathBuf, PathBuf)>,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Create {
        path: PathBuf,
        is_dir: bool,
    },
    Recycle {
        originals: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct OperationHistory {
    undo: Vec<FileOperation>,
    redo: Vec<FileOperation>,
}

impl OperationHistory {
    pub fn record(&mut self, op: FileOperation) {
        self.undo.push(op);
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn take_undo(&mut self) -> Option<FileOperation> {
        self.undo.pop()
    }

    pub fn take_redo(&mut self) -> Option<FileOperation> {
        self.redo.pop()
    }

    pub fn push_redo(&mut self, op: FileOperation) {
        self.redo.push(op);
    }

    pub fn push_undo(&mut self, op: FileOperation) {
        self.undo.push(op);
    }
}

/// Apply the inverse of `op` (undo).
pub fn apply_undo(op: &FileOperation) -> anyhow::Result<()> {
    match op {
        FileOperation::Move { moves } => {
            for (from, to) in moves.iter().rev() {
                move_path_back(to, from)?;
            }
        }
        FileOperation::Copy { copies } => {
            for (_, to) in copies.iter().rev() {
                if to.exists() {
                    recycle_paths(std::slice::from_ref(to))?;
                }
            }
        }
        FileOperation::Rename { from, to } => {
            let name = file_name(from)?;
            if to.exists() {
                rename_path(to, &name)?;
            }
        }
        FileOperation::Create { path, .. } => {
            if path.exists() {
                recycle_paths(std::slice::from_ref(path))?;
            }
        }
        FileOperation::Recycle { originals } => {
            restore_recycled_originals(originals)?;
        }
    }
    Ok(())
}

/// Re-apply `op` (redo).
pub fn apply_redo(op: &FileOperation) -> anyhow::Result<()> {
    match op {
        FileOperation::Move { moves } => {
            for (from, to) in moves {
                if from.exists() {
                    let parent = to
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("invalid move target {}", to.display()))?;
                    transfer_one(from, parent, ClipboardOperation::Cut, false)?;
                }
            }
        }
        FileOperation::Copy { copies } => {
            for (from, to) in copies {
                if from.exists() {
                    let parent = to
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("invalid copy target {}", to.display()))?;
                    transfer_one(from, parent, ClipboardOperation::Copy, false)?;
                }
            }
        }
        FileOperation::Rename { from, to } => {
            let name = file_name(to)?;
            if from.exists() {
                rename_path(from, &name)?;
            }
        }
        FileOperation::Create { path, is_dir } => {
            if path.exists() {
                return Ok(());
            }
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("invalid create path {}", path.display()))?;
            let name = file_name(path)?;
            if *is_dir {
                create_directory(parent, &name)?;
            } else {
                create_file(parent, &name)?;
            }
        }
        FileOperation::Recycle { originals } => {
            recycle_paths(originals)?;
        }
    }
    Ok(())
}

fn move_path_back(from: &Path, to: &Path) -> anyhow::Result<()> {
    if !from.exists() {
        anyhow::bail!("{} no longer exists", from.display());
    }
    let parent = to
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid path {}", to.display()))?;
    transfer_one(from, parent, ClipboardOperation::Cut, false)
}

fn file_name(path: &Path) -> anyhow::Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow::anyhow!("invalid path {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("cyberfiles_history_{label}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn move_undo_redo_round_trip() {
        let root = temp_dir("move");
        let src = root.join("a.txt");
        fs::write(&src, b"hi").unwrap();
        let dst_dir = root.join("dest");
        fs::create_dir_all(&dst_dir).unwrap();
        let dst = dst_dir.join("a.txt");

        transfer_one(&src, &dst_dir, ClipboardOperation::Cut, false).unwrap();
        assert!(!src.exists());
        assert!(dst.exists());

        let op = FileOperation::Move {
            moves: vec![(src.clone(), dst.clone())],
        };
        apply_undo(&op).unwrap();
        assert!(src.exists());
        assert!(!dst.exists());

        apply_redo(&op).unwrap();
        assert!(!src.exists());
        assert!(dst.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rename_undo_redo_round_trip() {
        let root = temp_dir("rename");
        let from = root.join("old.txt");
        fs::write(&from, b"x").unwrap();
        let to = root.join("new.txt");

        rename_path(&from, "new.txt").unwrap();
        assert!(!from.exists());
        assert!(to.exists());

        let op = FileOperation::Rename {
            from: from.clone(),
            to: to.clone(),
        };
        apply_undo(&op).unwrap();
        assert!(from.exists());
        assert!(!to.exists());

        apply_redo(&op).unwrap();
        assert!(!from.exists());
        assert!(to.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn create_undo_removes_path() {
        let root = temp_dir("create");
        let path = root.join("created.txt");
        create_file(&root, "created.txt").unwrap();
        assert!(path.exists());

        apply_undo(&FileOperation::Create {
            path: path.clone(),
            is_dir: false,
        })
        .unwrap();
        assert!(!path.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn history_stack_limits_and_clears_redo() {
        let mut history = OperationHistory::default();
        for i in 0..60 {
            history.record(FileOperation::Create {
                path: PathBuf::from(format!("/tmp/item{i}.txt")),
                is_dir: false,
            });
        }
        assert_eq!(history.undo.len(), MAX_HISTORY);
        history.record(FileOperation::Create {
            path: PathBuf::from("/tmp/redo_clear.txt"),
            is_dir: false,
        });
        history.push_redo(FileOperation::Create {
            path: PathBuf::from("/tmp/should_clear.txt"),
            is_dir: false,
        });
        history.record(FileOperation::Create {
            path: PathBuf::from("/tmp/new.txt"),
            is_dir: false,
        });
        assert!(history.redo.is_empty());
    }
}
