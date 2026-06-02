/// In-progress background file load shown in the editor-top progress bar.
pub(crate) struct FileLoadState {
    pub(crate) generation: u64,
    pub(crate) target_tab: usize,
    pub(crate) progress: f32,
}
