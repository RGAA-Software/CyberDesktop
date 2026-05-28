#[derive(Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
#[repr(transparent)]
pub struct ThreadId(pub i64);

impl From<i64> for ThreadId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

pub struct Session;
