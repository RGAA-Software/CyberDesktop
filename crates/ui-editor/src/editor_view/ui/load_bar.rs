//! Floating file-open progress bar at the top of the editor surface.

use super::super::imports::*;
use gpui_component::progress::Progress;

impl EngineEditor {
    pub(crate) fn render_file_load_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let load = self.file_load.as_ref()?;
        if load.target_tab != self.active {
            return None;
        }
        let theme = cx.theme();
        Some(
            div()
                .id("file-load-bar")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(3.0))
                .bg(theme.border)
                .child(
                    Progress::new("file-open-progress")
                        .w_full()
                        .h_full()
                        .value(load.progress * 100.0),
                ),
        )
    }
}
