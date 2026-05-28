use gpui::EventEmitter;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ThreadStatus {
    #[default]
    Running,
    Stopped,
    Stepping,
    Exited,
    Ended,
}

impl ThreadStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ThreadStatus::Running => "Running",
            ThreadStatus::Stopped => "Stopped",
            ThreadStatus::Stepping => "Stepping",
            ThreadStatus::Exited => "Exited",
            ThreadStatus::Ended => "Ended",
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
#[repr(transparent)]
pub struct ThreadId(pub i64);

impl From<i64> for ThreadId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

#[derive(Debug)]
pub enum SessionEvent {
    InvalidateInlineValue,
}

pub struct Session;

impl EventEmitter<SessionEvent> for Session {}

impl Session {
    pub fn any_stopped_thread(&self) -> bool {
        false
    }
}
