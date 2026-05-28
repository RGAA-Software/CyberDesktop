//! Stub terminal integration for Notepad++ / `cybereditor` builds (`ide-terminals` off).

use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use gpui::{App, Context, Entity, Task, WeakEntity};
use std::path::Path;
use task::SpawnInTerminal;

use crate::Project;

pub struct Terminal;

pub struct Terminals {
    pub(crate) local_handles: Vec<WeakEntity<Terminal>>,
}

impl Project {
    pub fn active_entry_directory(&self, cx: &App) -> Option<PathBuf> {
        let entry_id = self.active_entry()?;
        let worktree = self.worktree_for_entry(entry_id, cx)?;
        let worktree = worktree.read(cx);
        let entry = worktree.entry_for_id(entry_id)?;

        let absolute_path = worktree.absolutize(entry.path.as_ref());
        if entry.is_dir() {
            Some(absolute_path)
        } else {
            absolute_path.parent().map(|p| p.to_path_buf())
        }
    }

    pub fn active_project_directory(&self, cx: &App) -> Option<Arc<Path>> {
        self.active_entry()
            .and_then(|entry_id| self.worktree_for_entry(entry_id, cx))
            .into_iter()
            .chain(self.worktrees(cx))
            .find_map(|tree| tree.read(cx).root_dir())
    }

    pub fn first_project_directory(&self, cx: &App) -> Option<PathBuf> {
        let worktree = self.worktrees(cx).next()?;
        let worktree = worktree.read(cx);
        if worktree.root_entry()?.is_dir() {
            Some(worktree.abs_path().to_path_buf())
        } else {
            None
        }
    }

    pub fn create_terminal_task(
        &mut self,
        _spawn_task: SpawnInTerminal,
        _cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Terminal>>> {
        Task::ready(Err(anyhow!("terminals are disabled in this build")))
    }

    pub fn create_terminal_shell(
        &mut self,
        _cwd: Option<PathBuf>,
        _cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Terminal>>> {
        Task::ready(Err(anyhow!("terminals are disabled in this build")))
    }

    pub fn create_local_terminal(
        &mut self,
        _cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Terminal>>> {
        Task::ready(Err(anyhow!("terminals are disabled in this build")))
    }

    pub fn clone_terminal(
        &mut self,
        _terminal: &Entity<Terminal>,
        _cx: &mut Context<'_, Project>,
        _cwd: Option<PathBuf>,
    ) -> Task<Result<Entity<Terminal>>> {
        Task::ready(Err(anyhow!("terminals are disabled in this build")))
    }

    pub fn exec_in_shell(
        &self,
        _command: String,
        _cx: &mut Context<Self>,
    ) -> Task<Result<smol::process::Command>> {
        Task::ready(Err(anyhow!("terminals are disabled in this build")))
    }

    pub fn local_terminal_handles(&self) -> &Vec<WeakEntity<Terminal>> {
        &self.terminals.local_handles
    }
}
