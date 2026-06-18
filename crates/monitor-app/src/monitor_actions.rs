use gpui::{actions, Action};
use serde::Deserialize;

use crate::monitor_model::ProcessSortColumn;

actions!(monitor_process, [RefreshProcesses]);

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct TerminateProcess {
    pub pid: u32,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct RevealProcessExe {
    pub pid: u32,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct ShowProcessDetails {
    pub pid: u32,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct CycleProcessSort {
    pub column: ProcessSortColumn,
}

/// Views that can react to process-list context-menu actions.
pub trait ProcessActionHandler: Sized {
    fn terminate_process(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
    fn reveal_process_exe(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
    fn show_process_details(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
}
