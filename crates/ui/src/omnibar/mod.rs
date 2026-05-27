mod breadcrumb_bar;
mod breadcrumb_flyout;
mod breadcrumb_host;

pub use breadcrumb_host::OmnibarBreadcrumbCallbacks;

/// Drag-hover delay before navigating into a breadcrumb folder (ms). Files uses 1300ms.
/// Files `HoverToOpenTimespan` for drag-hover navigation into breadcrumb folders.
pub const BREADCRUMB_DRAG_HOVER_OPEN_MS: u64 = 1300;
