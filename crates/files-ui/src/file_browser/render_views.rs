use super::*;

#[path = "render_views/columns.rs"]
mod columns;
#[path = "render_views/table_list.rs"]
mod table_list;
#[path = "render_views/tiles.rs"]
mod tiles;

impl FileBrowser {
    pub(super) fn dismiss_main_page_path_edit_if_active(cx: &mut Context<Self>) {
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            nav.main_page().update(cx, |page, cx| {
                page.dismiss_omnibar_path_edit(cx);
                page.dismiss_omnibar_search_mode(cx);
            });
        }
    }

    pub(super) fn file_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.schedule_list_icon_warm(window, cx);
        match self.view_mode {
            ViewMode::Details => self.details_table(window, cx).into_any_element(),
            ViewMode::List => self.list_view(window, cx).into_any_element(),
            ViewMode::Grid => self.grid_view(window, cx).into_any_element(),
            ViewMode::Cards => self.cards_view(window, cx).into_any_element(),
            ViewMode::Columns => self.columns_view(window, cx).into_any_element(),
        }
    }
}
