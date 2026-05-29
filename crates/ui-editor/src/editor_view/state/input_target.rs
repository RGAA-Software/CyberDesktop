/// Where typed text currently goes. Find / Find-in-Files / Go to Line use
/// gpui-component [`InputState`] widgets that handle their own keyboard input.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputTarget {
    Document,
}
