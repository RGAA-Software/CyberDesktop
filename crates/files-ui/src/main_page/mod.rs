use files_core::{
    load_config, load_session_tabs, APP_NAME,
};
use files_fs::OmnibarPathSuggestion;

const MAX_CLOSED_TABS: usize = 12;
use files_commands::{
    CloseActivePane, CopyItems, CutItems, FocusOmnibar, FocusOtherPane, FocusSearch, NavigateBack,
    NavigateForward, NavigateUp, PasteItems, ReopenClosedTab, RefreshDirectory, RedoOperation,
    ArrangePanesHorizontally, ArrangePanesVertically, SelectAll, SplitPaneHorizontally,
    SplitPaneVertically, ToggleDualPane, UndoOperation,
    FILE_BROWSER,
};
use gpui::{prelude::*, *};
use gpui_component::{input::InputState, v_flex};

use crate::app_state::AppOperationHistory;
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
mod settings_overlay;
mod sidebar;
mod tab_bar_menu;
mod tabs;

/// Navigation bar height (design V11).
const NAV_TOOLBAR_HEIGHT: Pixels = px(52.);
/// Title-bar tab chip height inside the 46px chrome row.
const TITLE_TAB_BAR_HEIGHT: Pixels = px(34.);
/// Tab width bounds in the title bar (label truncates inside).
const TITLE_TAB_MIN_WIDTH: Pixels = px(140.);
const TITLE_TAB_WIDTH: Pixels = px(220.);
/// Omnibar / breadcrumb bar height in the navigation row.
const OMNIBAR_BAR_HEIGHT: Pixels = px(34.);

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
    omnibar_search_mode: bool,
    omnibar_search_input: Option<Entity<InputState>>,
    _omnibar_search_subscription: Option<Subscription>,
    omnibar_path_input: Option<Entity<InputState>>,
    _omnibar_path_subscription: Option<Subscription>,
    omnibar_suggestions: Vec<OmnibarPathSuggestion>,
    omnibar_suggestion_index: Option<usize>,
    omnibar_suggestions_open: bool,
    omnibar_suggestions_generation: u64,
    omnibar_path_input_anchor: Option<Point<Pixels>>,
    omnibar_path_blur_generation: u64,
    omnibar_search_suggestions: Vec<String>,
    omnibar_search_suggestion_index: Option<usize>,
    omnibar_search_suggestions_open: bool,
    omnibar_search_suggestions_generation: u64,
    omnibar_search_input_anchor: Option<Point<Pixels>>,
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
    pending_settings_toggle: bool,
    tab_bar_popup_menu: Option<tab_bar_menu::TabBarPopupMenuState>,
}

impl MainPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = load_config().unwrap_or_default();
        let show_info_pane = config.show_info_pane;
        let session_tabs = load_session_tabs();
        let (tabs, active_tab, next_tab_id) = if !config.auto_restore_tabs || session_tabs.is_empty() {
            let shell = cx.new(|cx| ShellPanes::new(cx, NavigationTarget::Home));
            (vec![TabEntry { id: 0, shell }], 0, 1)
        } else {
            let mut restored = Vec::with_capacity(session_tabs.len());
            for (id, encoded) in session_tabs.iter().enumerate() {
                let layout = config.session_pane_layouts.get(id).cloned();
                let primary_target = layout
                    .as_ref()
                    .filter(|l| !l.primary_tab.is_empty())
                    .map(|l| Self::decode_session_target(l.primary_tab.as_str()))
                    .unwrap_or_else(|| Self::decode_session_target(encoded));
                let shell = cx.new(|cx| {
                    let mut shell = ShellPanes::new(cx, primary_target);
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
            (restored, 0, next_id)
        };
        let this = Self {
            focus_handle: cx.focus_handle(),
            tabs,
            active_tab,
            next_tab_id,
            tab_bar_scroll_handle: ScrollHandle::new(),
            pending_tab_scroll_to_ix: Some(active_tab),
            show_info_pane,
            info_pane: cx.new(|cx| InfoPane::new(cx)),
            omnibar_show_full_path: false,
            omnibar_search_mode: false,
            omnibar_search_input: None,
            _omnibar_search_subscription: None,
            omnibar_path_input: None,
            _omnibar_path_subscription: None,
            omnibar_suggestions: Vec::new(),
            omnibar_suggestion_index: None,
            omnibar_suggestions_open: false,
            omnibar_suggestions_generation: 0,
            omnibar_path_input_anchor: None,
            omnibar_path_blur_generation: 0,
            omnibar_search_suggestions: Vec::new(),
            omnibar_search_suggestion_index: None,
            omnibar_search_suggestions_open: false,
            omnibar_search_suggestions_generation: 0,
            omnibar_search_input_anchor: None,
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
            pending_settings_toggle: false,
            tab_bar_popup_menu: None,
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
        crate::app_state::AppOperationHistory::init(cx);
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
        self.flush_pending_settings_toggle(cx);
        self.adapt_active_shell_viewport(window.viewport_size().width, cx);
        let active_shell = self.active_shell();
        let show_info_pane = self.show_info_pane;
        let show_nav = !matches!(
            self.active_navigation_target(cx),
            NavigationTarget::Home | NavigationTarget::Settings
        );
        let file_navigation_active = self.file_navigation_active(cx);
        let (info_selection, info_read_options) = self.info_pane_update(cx);
        self.info_pane.update(cx, |pane, cx| {
            pane.set_selection(info_selection, info_read_options, cx);
        });

        v_flex()
            .id("main-page")
            .relative()
            .size_full()
            .min_h_0()
            .min_w_0()
            .track_focus(&self.focus_handle)
            .when(file_navigation_active, |page| {
                page.key_context(FILE_BROWSER)
            })
            .on_action(cx.listener(|this, _: &FocusOmnibar, window, cx| {
                this.enter_omnibar_path_edit(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.enter_omnibar_search_mode(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &NavigateUp, _, cx| {
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_up(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _, cx| {
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_back(cx));
            }))
            .on_action(cx.listener(|this, _: &NavigateForward, _, cx| {
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
                    return;
                }
                this.active_file_browser(cx)
                    .update(cx, |browser, cx| browser.go_forward(cx));
            }))
            .on_action(cx.listener(|this, _: &RefreshDirectory, window, cx| {
                if this.omnibar_path_edit_active() || this.omnibar_search_mode_active() {
                    return;
                }
                if window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                this.refresh_active_view(cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &UndoOperation, window, cx| {
                if this.omnibar_path_edit_active() || this.omnibar_search_mode_active() {
                    return;
                }
                let browser = this.active_file_browser(cx);
                let in_rename = browser.read(cx).is_renaming();
                if !in_rename && window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                if !AppOperationHistory::can_undo(cx) || !this.file_navigation_active(cx) {
                    return;
                }
                let browser = this.active_file_browser(cx);
                crate::file_ops_history::spawn_history_undo(browser, window, cx);
                this.refresh_active_view(cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &RedoOperation, window, cx| {
                if this.omnibar_path_edit_active() || this.omnibar_search_mode_active() {
                    return;
                }
                let browser = this.active_file_browser(cx);
                let in_rename = browser.read(cx).is_renaming();
                if !in_rename && window.context_stack().iter().any(|ctx| ctx.contains("Input")) {
                    return;
                }
                if !AppOperationHistory::can_redo(cx) || !this.file_navigation_active(cx) {
                    return;
                }
                let browser = this.active_file_browser(cx);
                crate::file_ops_history::spawn_history_redo(browser, window, cx);
                this.refresh_active_view(cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
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
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
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
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
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
                if !this.file_navigation_active(cx)
                    || this.omnibar_path_edit_active()
                    || this.omnibar_search_mode_active()
                {
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
            .on_action(cx.listener(|this, _: &ToggleDualPane, _, cx| {
                this.toggle_dual_pane(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusOtherPane, window, cx| {
                this.focus_other_pane(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseActivePane, _, cx| {
                this.close_active_pane(cx);
            }))
            .on_action(cx.listener(|this, _: &SplitPaneVertically, _, cx| {
                this.split_pane_vertically(cx);
            }))
            .on_action(cx.listener(|this, _: &SplitPaneHorizontally, _, cx| {
                this.split_pane_horizontally(cx);
            }))
            .on_action(cx.listener(|this, _: &ArrangePanesVertically, _, cx| {
                this.arrange_panes_vertically(cx);
            }))
            .on_action(cx.listener(|this, _: &ArrangePanesHorizontally, _, cx| {
                this.arrange_panes_horizontally(cx);
            }))
            .when_some(self.tab_bar_popup_overlay(), |page, overlay| page.child(overlay))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.omnibar_search_mode_active() {
                    this.on_omnibar_search_key_down(event, window, cx);
                    return;
                }
                if this.omnibar_path_edit_active() {
                    this.on_omnibar_path_key_down(event, window, cx);
                    return;
                }
                if event.keystroke.key.as_str() == "escape" {
                    if this.show_status_center {
                        this.show_status_center = false;
                        cx.notify();
                    } else if matches!(
                        this.active_navigation_target(cx),
                        NavigationTarget::SearchResults { .. }
                    ) {
                        let browser = this.active_file_browser(cx);
                        if browser.read(cx).can_go_back() {
                            cx.defer(move |cx| {
                                browser.update(cx, |browser, cx| {
                                    browser.go_back(cx);
                                });
                            });
                            cx.stop_propagation();
                        }
                    } else {
                        this.dismiss_omnibar_path_edit(cx);
                    }
                }
            }))
            .child(self.render_title_bar(window, cx))
            .child(
                div()
                    .id("main-body")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(self.render_shell_layout_row(
                        window,
                        active_shell,
                        show_info_pane,
                        show_nav,
                        cx,
                    )),
            )
    }
}
