use gpui::{prelude::*, *};

use crate::shell::{append_dual_pane_popup_menu, dual_pane_menu_state, DualPanePopupProfile};
use app_ui::popup_menu::PopupMenu;

use super::MainPage;

pub(super) struct TabBarPopupMenuState {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

impl MainPage {
    pub(super) fn close_tab_bar_popup_menu(&mut self) {
        self.tab_bar_popup_menu = None;
    }

    pub(super) fn open_tab_bar_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tab_bar_popup_menu();

        let state = dual_pane_menu_state(cx);
        let page = cx.entity();

        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            append_dual_pane_popup_menu(menu, window, cx, state, DualPanePopupProfile::TabBar)
        });

        let subscription = window.subscribe(&menu, cx, {
            let page = page.clone();
            move |_, _: &DismissEvent, window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.close_tab_bar_popup_menu();
                    cx.notify();
                });
                window.refresh();
            }
        });

        self.tab_bar_popup_menu = Some(TabBarPopupMenuState {
            position,
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(super) fn tab_bar_popup_overlay(&self) -> Option<impl IntoElement> {
        let state = self.tab_bar_popup_menu.as_ref()?;
        let position = state.position;
        let menu = state.menu.clone();
        Some(
            deferred(
                anchored()
                    .position(position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(1),
        )
    }
}
