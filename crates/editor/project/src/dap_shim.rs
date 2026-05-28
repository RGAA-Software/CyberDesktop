//! Minimal stand-ins for `dap` types when `remote-debug` is disabled.

use gpui::SharedString;
use std::ops::Deref;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DebugAdapterName(pub SharedString);

impl Deref for DebugAdapterName {
    type Target = SharedString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for DebugAdapterName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<SharedString> for DebugAdapterName {
    fn from(name: SharedString) -> Self {
        Self(name)
    }
}

impl From<&str> for DebugAdapterName {
    fn from(name: &str) -> Self {
        Self(SharedString::from(name))
    }
}

pub mod client {
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub struct SessionId(pub u32);

    impl SessionId {
        pub fn from_proto(client_id: u64) -> Self {
            Self(client_id as u32)
        }

        pub fn to_proto(self) -> u64 {
            self.0 as u64
        }
    }
}

pub type StackFrameId = u64;

pub mod inline_value {
  #[derive(Clone, Debug)]
  pub struct InlineValueLocation {
      pub variable_name: String,
      pub scope: VariableScope,
      pub lookup: VariableLookupKind,
      pub row: usize,
      pub column: usize,
  }

  #[derive(Clone, Copy, Debug)]
  pub enum VariableScope {
      Local,
      Global,
  }

  #[derive(Clone, Copy, Debug)]
  pub enum VariableLookupKind {
      Variable,
  }
}

pub struct DebugAdapterClient;
