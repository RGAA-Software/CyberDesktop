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

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct StartServiceAction {
    pub name: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct StopServiceAction {
    pub name: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct RestartServiceAction {
    pub name: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct RevealStartupItem {
    pub command: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct SetProcessPriority {
    pub pid: u32,
    pub priority: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct SetProcessIoPriority {
    pub pid: u32,
    pub priority: String,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct SetProcessAffinity {
    pub pid: u32,
    pub affinity_mask: u64,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct SuspendProcess {
    pub pid: u32,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct ResumeProcess {
    pub pid: u32,
}

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = monitor_process, no_json)]
pub struct TerminateProcessTree {
    pub pid: u32,
}

/// Views that can react to process-list context-menu actions.
pub trait ProcessActionHandler: Sized {
    fn terminate_process(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
    fn reveal_process_exe(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
    fn show_process_details(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) {}
    fn start_service(&mut self, _name: &str, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
    fn stop_service(&mut self, _name: &str, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
    fn restart_service(&mut self, _name: &str, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
    fn reveal_startup_item(&mut self, _command: &str, _cx: &mut gpui::Context<Self>) {}
    fn set_process_priority(
        &mut self,
        _pid: u32,
        _priority: &str,
        _cx: &mut gpui::Context<Self>,
    ) -> bool {
        false
    }
    fn set_process_io_priority(
        &mut self,
        _pid: u32,
        _priority: &str,
        _cx: &mut gpui::Context<Self>,
    ) -> bool {
        false
    }
    fn set_process_affinity(
        &mut self,
        _pid: u32,
        _mask: u64,
        _cx: &mut gpui::Context<Self>,
    ) -> bool {
        false
    }
    fn suspend_process(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
    fn resume_process(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
    fn terminate_process_tree(&mut self, _pid: u32, _cx: &mut gpui::Context<Self>) -> bool {
        false
    }
}
