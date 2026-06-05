use files_core::{home_widget_prefs, save_home_widget_prefs, HomeWidgetPrefs};
use files_fs::{
    file_tag_previews, list_drives, list_quick_access_entries, list_recent_files,
    load_home_file_tags, quick_access_automatic_destinations_dir, DirectoryWatcher, DriveInfo,
    FileTagPreview, QuickAccessEntry, RecentItem,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    anchored, deferred, div, prelude::*, px, Anchor, DismissEvent, Entity, MouseButton,
    MouseDownEvent, Pixels, Point, Subscription, Task, Window,
};
use gpui_component::{v_flex, ActiveTheme as _, ElementExt as _};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::home::widget_shell::{HOME_PAGE_PADDING_X, HOME_PAGE_PADDING_Y, HOME_SECTION_GAP};
use crate::home::widgets::{load_network_entries, NetworkEntry};
use crate::shell::{append_dual_pane_popup_menu, dual_pane_menu_state, DualPanePopupProfile};
use app_ui::popup_menu::{PopupMenu, PopupMenuItem};

/// Loaded Home dashboard data (Files `RefreshWidgetProperties` snapshot).
#[derive(Clone)]
pub struct HomeSnapshot {
    pub quick_access: Vec<QuickAccessEntry>,
    pub drives: Vec<DriveInfo>,
    pub network: Vec<NetworkEntry>,
    pub tag_previews: Vec<FileTagPreview>,
    pub recent: Vec<RecentItem>,
}

impl HomeSnapshot {
    fn load() -> Self {
        let tags = load_home_file_tags();
        Self {
            quick_access: list_quick_access_entries(),
            drives: list_drives(),
            network: load_network_entries(),
            tag_previews: file_tag_previews(&tags),
            recent: list_recent_files(),
        }
    }
}

struct HomePopupMenuState {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

pub struct HomePage {
    pub(super) prefs: HomeWidgetPrefs,
    snapshot: Option<HomeSnapshot>,
    loading: bool,
    load_generation: u64,
    popup_menu: Option<HomePopupMenuState>,
    #[cfg(windows)]
    _qa_watcher: Option<DirectoryWatcher>,
    #[cfg(windows)]
    _qa_watch_task: Option<Task<()>>,
    /// Shell thumbnail PNG bytes for Home cards (path key → image).
    pub(super) thumbnail_bytes: HashMap<String, Arc<Vec<u8>>>,
    pub(super) thumbnail_pending: HashSet<String>,
    /// Measured inner width of the home scroll column (drives card/tag grids).
    content_width: Option<Pixels>,
}

impl HomePage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            prefs: home_widget_prefs(),
            snapshot: None,
            loading: false,
            load_generation: 0,
            popup_menu: None,
            #[cfg(windows)]
            _qa_watcher: None,
            #[cfg(windows)]
            _qa_watch_task: None,
            thumbnail_bytes: HashMap::new(),
            thumbnail_pending: HashSet::new(),
            content_width: None,
        };
        page.schedule_load(cx);
        #[cfg(windows)]
        page.start_quick_access_watcher(cx);
        page
    }

    #[cfg(windows)]
    fn start_quick_access_watcher(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = quick_access_automatic_destinations_dir() else {
            return;
        };
        if !dir.is_dir() {
            return;
        }
        let Ok((watcher, events)) =
            DirectoryWatcher::watch_recursive(&dir, Duration::from_millis(800))
        else {
            return;
        };
        self._qa_watcher = Some(watcher);
        self._qa_watch_task = Some(cx.spawn(async move |page, cx| {
            while events.recv_async().await.is_ok() {
                let _ = page.update(cx, |page, cx| {
                    page.reload(cx);
                    AppNavigation::refresh_quick_access(cx);
                });
            }
        }));
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.snapshot = None;
        self.thumbnail_bytes.clear();
        self.thumbnail_pending.clear();
        self.schedule_load(cx);
    }

    fn schedule_load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        cx.spawn(async move |page, cx| {
            let snapshot = cx
                .background_spawn(async move { HomeSnapshot::load() })
                .await;
            let _ = page.update(cx, |page, cx| {
                if page.load_generation != generation {
                    return;
                }
                page.snapshot = Some(snapshot);
                page.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_expanded(&mut self, section: &str, cx: &mut Context<Self>) {
        match section {
            "quick_access" => self.prefs.quick_access_expanded = !self.prefs.quick_access_expanded,
            "drives" => self.prefs.drives_expanded = !self.prefs.drives_expanded,
            "network" => self.prefs.network_expanded = !self.prefs.network_expanded,
            "file_tags" => self.prefs.file_tags_expanded = !self.prefs.file_tags_expanded,
            "recent" => self.prefs.recent_expanded = !self.prefs.recent_expanded,
            _ => {}
        }
        let _ = save_home_widget_prefs(&self.prefs);
        cx.notify();
    }

    fn close_popup_menu(&mut self) {
        self.popup_menu = None;
    }

    fn open_popup_menu(
        &mut self,
        position: Point<Pixels>,
        menu: Entity<PopupMenu>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_popup_menu();

        let page = cx.entity();
        let subscription = window.subscribe(&menu, cx, {
            move |_, _: &DismissEvent, window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.close_popup_menu();
                    cx.notify();
                });
                window.refresh();
            }
        });

        self.popup_menu = Some(HomePopupMenuState {
            position,
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn open_widget_prefs_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = PopupMenu::build(window, cx, |menu, window, cx| {
            build_page_context_menu(menu, &home_widget_prefs(), window, cx)
        });
        self.open_popup_menu(position, menu, window, cx);
    }

    fn move_widget_in_order(&mut self, section: &str, up: bool, cx: &mut Context<Self>) {
        self.prefs.move_widget(section, up);
        let _ = save_home_widget_prefs(&self.prefs);
        cx.notify();
    }

    pub(super) fn open_section_menu(
        &mut self,
        section_key: &'static str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let order = self.prefs.widget_order_normalized();
        let pos = order.iter().position(|id| id == section_key).unwrap_or(0);
        let can_move_up = pos > 0;
        let can_move_down = pos + 1 < order.len();

        let page = cx.entity();
        let menu = PopupMenu::build(window, cx, move |menu, _window, _cx| {
            menu.item(
                PopupMenuItem::new(t!("home.widget.move_up"))
                    .disabled(!can_move_up)
                    .on_click({
                        let page = page.clone();
                        move |_, _, cx| {
                            let _ = page.update(cx, |page, cx| {
                                page.move_widget_in_order(section_key, true, cx);
                            });
                            cx.stop_propagation();
                        }
                    }),
            )
            .item(
                PopupMenuItem::new(t!("home.widget.move_down"))
                    .disabled(!can_move_down)
                    .on_click({
                        let page = page.clone();
                        move |_, _, cx| {
                            let _ = page.update(cx, |page, cx| {
                                page.move_widget_in_order(section_key, false, cx);
                            });
                            cx.stop_propagation();
                        }
                    }),
            )
        });
        self.open_popup_menu(position, menu, window, cx);
    }

    fn on_blank_right_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Right {
            return;
        }
        self.open_widget_prefs_menu(event.position, window, cx);
    }

    pub(super) fn layout_width(&self, window: &Window) -> Pixels {
        self.content_width
            .unwrap_or_else(|| crate::home::widget_shell::estimated_content_width(window))
    }
}

fn build_page_context_menu(
    menu: PopupMenu,
    prefs: &HomeWidgetPrefs,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let mut menu = menu;
    let items = [
        (
            "quick_access",
            t!("home.widget.quick_access"),
            prefs.show_quick_access,
        ),
        ("drives", t!("home.widget.drives"), prefs.show_drives),
        ("network", t!("home.widget.network"), prefs.show_network),
        ("file_tags", t!("home.widget.tags"), prefs.show_file_tags),
        ("recent", t!("home.widget.recent"), prefs.show_recent),
    ];
    for (key, label, checked) in items {
        let suffix = if checked { " ✓" } else { "" };
        let text = format!("{label}{suffix}");
        let key = key.to_string();
        menu = menu.item(PopupMenuItem::new(text).on_click(move |_, _, cx| {
            let mut prefs = home_widget_prefs();
            match key.as_str() {
                "quick_access" => prefs.show_quick_access = !prefs.show_quick_access,
                "drives" => prefs.show_drives = !prefs.show_drives,
                "network" => prefs.show_network = !prefs.show_network,
                "file_tags" => prefs.show_file_tags = !prefs.show_file_tags,
                "recent" => prefs.show_recent = !prefs.show_recent,
                _ => {}
            }
            let _ = save_home_widget_prefs(&prefs);
            AppNavigation::refresh_home_widgets(cx);
            cx.stop_propagation();
        }));
    }
    let state = dual_pane_menu_state(cx);
    if state.multi_pane_available || state.dual {
        menu = menu.separator();
        menu = append_dual_pane_popup_menu(
            menu,
            window,
            cx,
            state,
            DualPanePopupProfile::PageSurface,
        );
    }
    menu
}

impl Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prefs = home_widget_prefs();
        if self.snapshot.is_none() && !self.loading {
            self.schedule_load(cx);
        }

        let snapshot = self.snapshot.clone().unwrap_or_else(|| HomeSnapshot {
            quick_access: Vec::new(),
            drives: Vec::new(),
            network: Vec::new(),
            tag_previews: Vec::new(),
            recent: Vec::new(),
        });

        let widget_order = self.prefs.widget_order_normalized();

        let page_entity = cx.entity().clone();

        let menu_overlay = self.popup_menu.as_ref().map(|state| {
            let position = state.position;
            let menu = state.menu.clone();
            deferred(
                anchored()
                    .position(position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(1)
        });

        // NOTE: Do not use `.context_menu()` on this column — it wraps all descendants and
        // stacks the widget-visibility menu on top of drive/file item menus.
        div()
            .id("home-page")
            .relative()
            .size_full()
            .min_h_0()
            .when_some(menu_overlay, |page, overlay| page.child(overlay))
            .child(
                v_flex()
                    .id("home-page-scroll")
                    .size_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(HOME_PAGE_PADDING_X)
                    .py(HOME_PAGE_PADDING_Y)
                    .gap(HOME_SECTION_GAP)
                    .bg(cx.theme().background)
                    .on_prepaint({
                        let page_entity = page_entity.clone();
                        move |bounds, _, cx| {
                            let width = bounds.size.width;
                            let _ = page_entity.update(cx, |page, cx| {
                                if page.content_width != Some(width) {
                                    page.content_width = Some(width);
                                    cx.notify();
                                }
                            });
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|page, event: &MouseDownEvent, window, cx| {
                            page.on_blank_right_click(event, window, cx);
                        }),
                    )
                    .when(self.loading && self.snapshot.is_none(), |page| {
                        page.child(div().child(t!("home.loading")))
                    })
                    .children(widget_order.iter().filter_map(|widget_id| {
                        if !self.prefs.is_widget_visible(widget_id) {
                            return None;
                        }
                        Some(match widget_id.as_str() {
                            "quick_access" => self
                                .render_quick_access_widget(window, cx, &snapshot.quick_access)
                                .into_any_element(),
                            "drives" => self
                                .render_drives_widget(window, cx, &snapshot.drives)
                                .into_any_element(),
                            "network" => self
                                .render_network_widget(window, cx, &snapshot.network)
                                .into_any_element(),
                            "file_tags" => self
                                .render_file_tags_widget(window, cx, &snapshot.tag_previews)
                                .into_any_element(),
                            "recent" => self
                                .render_recent_widget(window, cx, &snapshot.recent)
                                .into_any_element(),
                            _ => return None,
                        })
                    }))
                    .child(div().w_full().flex_1().min_h(px(64.))),
            )
    }
}
