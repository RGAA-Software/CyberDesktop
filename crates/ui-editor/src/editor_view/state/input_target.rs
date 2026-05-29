/// Where typed text currently goes. The Find / Find-in-Files panels use real
/// gpui-component inputs, so only the document and the lightweight Go to Line
/// overlay route text through the editor itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputTarget {
    Document,
    GotoLine,
}
