use cyber_desktop_core::{
    load_config, APP_NAME,
};

const MAX_CLOSED_TABS: usize = 12;
use cyber_desktop_commands::{
    CopyItems, CutItems, FocusOmnibar, NavigateBack, NavigateForward, NavigateUp, PasteItems,
    ReopenClosedTab, SelectAll, FILE_BROWSER,
};
use gpui::{prelude::*, *};
use gpui_component::{
    input::InputState,
    v_flex,
};

use crate::info_pane::InfoPane;
use crate::omnibar::OmnibarBreadcrumbCallbacks;
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::ReopenClosedTabAt;
use crate::shell::ShellPanes;
use crate::sidebar::SidebarSection;

mod omnibar;
mod info;
mod navigation;
mod render;
mod render_shell;
mod session;
mod sidebar;
mod tabs;

/// Matches Files `NavigationToolbar` height.
const NAV_TOOLBAR_HEIGHT: Pixels = px(48.);
/// Default medium `TabBar` height in the integrated title bar.
const TITLE_TAB_BAR_HEIGHT: Pixels = px(32.);
/// Fixed width per document tab in the title bar (label truncates inside).
const TITLE_TAB_WIDTH: Pixels = px(200.);
const TITLE_TAB_CLOSE_RIGHT_INSET: Pixels = px(5.);
/// Omnibar height (Files `AddressToolbarButtonStyle` uses 32px).
const OMNIBAR_BAR_HEIGHT: Pixels = px(32.);

struct TabEntry {
    id: u64,
    shell: Entity<ShellPanes>,
}

pub struct MainPage {
    focus_handle: FocusHandle,
    tabs: Vec<TabEntry>,
    active_tab: usize,
    next_tab_id: u64,
    tab_bar_scroll_handle: ScrollHandle,
    pending_tab_scroll_to_ix: Option<usize>,
    show_info_pane: bool,
    info_pane: Entity<InfoPane>,
    /// When true, show an editable path field instead of breadcrumb segments.
    omnibar_show_full_path: bool,
    omnibar_path_input: Option<Entity<InputState>>,
    _omnibar_path_subscription: Option<Subscription>,
    omnibar_breadcrumb_callbacks: Option<OmnibarBreadcrumbCallbacks>,
    omnibar_breadcrumb_width: f32,
    breadcrumb_drag_generation: u64,
    search_input: Option<Entity<InputState>>,
    _search_subscription: Option<Subscription>,
    sidebar_sections: Vec<SidebarSection>,
    sidebar_cache_key: u64,
    sidebar_cache_generation: u64,
    sidebar_cache_loading: bool,
    show_status_center: bool,
}

impl MainPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = load_config().unwrap_or_default();
        let show_info_pane = config.show_info_pane;
        let (tabs, active_tab, next_tab_id) = if config.session_tabs.is_empty() {
            let shell = cx.new(|cx| ShellPanes::new(cx, NavigationTarget::Home));
            (vec![TabEntry { id: 0, shell }], 0, 1)
        } else {
            let active = config
                .session_active_tab
                .min(config.session_tabs.len().saturating_sub(1));
            let mut restored = Vec::with_capacity(config.session_tabs.len());
            for (id, encoded) in config.session_tabs.iter().enumerate() {
                let target = Self::decode_session_target(encoded);
                let layout = config.session_pane_layouts.get(id).cloned();
                let shell = cx.new(|cx| {
                    let mut shell = ShellPanes::new(cx, target);
                    if let Some(ref layout) = layout {
                        shell.restore_layout(layout, Self::decode_session_target, cx);
                    }
                    shell
                });
                restored.push(TabEntry {
                    id: id as u64,
                    shell,
                });
            }
            let next_id = restored.len() as u64;
            (restored, active, next_id)
        };
        let this = Self {
            focus_handle: cx.focus_handle(),
            tabs,
            active_tab,
            next_tab_id,
            tab_bar_scroll_handle: ScrollHandle::new(),
            pending_tab_scroll_to_ix: Some(active_tab),
            show_info_pane,
            info_pane: cx.new(|_| InfoPane::new()),
            omnibar_show_full_path: false,
            omnibar_path_input: None,
            _omnibar_path_subscription: None,
            omnibar_breadcrumb_callbacks: None,
            omnibar_breadcrumb_width: 320.,
            breadcrumb_drag_generation: 0,
            search_input: None,
            _search_subscription: None,
            sidebar_sections: Vec::new(),
            sidebar_cache_key: 0,
            sidebar_cache_generation: 0,
            sidebar_cache_loading: false,
            show_status_center: false,
        };
        // Propagate initial show_info_pane to all file browsers.
        for tab in &this.tabs {
            let shell = tab.shell.clone();
            let panes = {
                let shell_ref = shell.read(cx);
                let mut panes = Vec::new();
                shell_ref.for_each_pane(|pane| {
                    panes.push(pane.clone());
                });
                panes
            };
            for pane in panes {
                let file_browser = pane.read(cx).file_browser();
                file_browser.update(cx, |browser, cx| {
                    browser.set_show_info_pane(show_info_pane, cx);
                });
            }
        }
        this
    }

    pub fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        app_menus::init(APP_NAME, cx);
        crate::app_state::TransferStatusGlobal::init(cx);
        let page = cx.new(|cx| Self::new(cx));
        crate::app_state::AppNavigation::set(page.clone(), cx);
        page
    }

}

impl Focusable for MainPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_sidebar_cache(cx);
        let active_shell = self.active_shell();
        let show_info_pane = self.show_info_pane;
        let file_navigation_active = self.file_navigation_active(cx);
        let info_item = self.info_selection(cx);
        self.info_pane
            .update(cx, |pane, _| pane.set_item(info_item));

        v_flex()
            .id("main-page")
            .size_full()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus_handle)
            .when(file_navigation_active, |page| {
                page.key_context(FILE_BROWSER)
            })
            .on_action(cx.listener(|this, _: &FocusOmnibar, window, cx| {
                this.focus_search_input(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NavigateUp, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_up(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_back(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateForward, _, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_forward(cx));
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.select_all();
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &CopyItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.copy_items(cx);
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &CutItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.cut_items(cx);
                    cx.notify();
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &PasteItems, window, cx| {
                if !this.file_navigation_active(cx) || this.omnibar_path_edit_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                let active_browser = this.active_file_browser(cx);
                active_browser.update(cx, |browser, cx| {
                    browser.paste_items(window, cx);
                });
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, _, cx| {
                this.reopen_closed_tab(cx);
            }))
            .on_action(cx.listener(|this, action: &ReopenClosedTabAt, _, cx| {
                this.reopen_closed_tab_at(action.index, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    if this.show_status_center {
                        this.show_status_center = false;
                        cx.notify();
                    } else {
                        this.dismiss_omnibar_path_edit(cx);
                    }
                }
            }))
            .child(self.render_title_bar(window, cx))
            .child(self.render_navigation_toolbar(window, cx))
            .child(
                div()
                    .id("main-body")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.render_shell_layout_row(window, active_shell, show_info_pane, cx)),
            )
    }
}
