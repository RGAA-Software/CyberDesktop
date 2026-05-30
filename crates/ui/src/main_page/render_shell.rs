use gpui::{prelude::*, *};
use gpui_component::{
    button::Button,
    h_flex,
    label::Label,
    v_flex,
    ActiveTheme as _,
    Size,
    Sizable as _,
};
use rust_i18n::t;

use super::MainPage;
use crate::app_state::{AppFileClipboard, TransferStatusGlobal};
use crate::resizable::{h_resizable, resizable_panel, ResizableState};
use crate::shell::navigation::NavigationTarget;
use crate::shell::ShellPanes;
use crate::sidebar::render_sidebar;
use cyber_desktop_commands::PasteItems;

impl MainPage {
    pub(super) fn render_content_column(
        &mut self,
        window: &mut Window,
        active_shell: Entity<ShellPanes>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = window;
        let mut col = v_flex()
            .id("content-column")
            .size_full()
            .min_h_0()
            .min_w_0()
            .relative()
            .child(
                div()
                    .id("main-content")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(active_shell),
            )
            .child(self.render_shelf_pane(cx))
            .child(div().flex_shrink_0().child(self.render_status_bar(cx)));

        if self.show_status_center {
            let page = cx.entity();
            let on_close = move |_window: &mut Window, cx: &mut App| {
                let _ = page.update(cx, |page, cx| {
                    page.show_status_center = false;
                    cx.notify();
                });
            };
            col = col.child(
                div()
                    .id("status-center-overlay")
                    .absolute()
                    .bottom(px(36.))
                    .right(px(8.))
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .child(crate::status_center::render_status_center_panel(cx, on_close)),
            );
        }

        col
    }

    /// Files `ShelfPane`: shows in-app clipboard staging above the status bar.
    fn render_shelf_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(clipboard) = AppFileClipboard::peek(cx) else {
            return div().id("shelf-pane").flex_shrink_0();
        };
        if clipboard.is_empty() {
            return div().id("shelf-pane").flex_shrink_0();
        }

        let count = clipboard.paths.len();
        let operation_label = match clipboard.operation {
            cyber_desktop_fs::ClipboardOperation::Copy => {
                t!("files.shelf.copying", count = count).to_string()
            }
            cyber_desktop_fs::ClipboardOperation::Cut => {
                t!("files.shelf.cutting", count = count).to_string()
            }
        };
        let preview = clipboard
            .paths
            .first()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .map(|name| {
                if count <= 1 {
                    t!("files.shelf.preview_one", name = name).to_string()
                } else {
                    t!("files.shelf.preview_many", name = name, rest = count - 1).to_string()
                }
            })
            .unwrap_or_default();

        h_flex()
            .id("shelf-pane")
            .flex_shrink_0()
            .h(px(36.))
            .w_full()
            .px_3()
            .gap_2()
            .items_center()
            .overflow_hidden()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                Label::new(operation_label)
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .flex_shrink_0(),
            )
            .when(!preview.is_empty(), |row| {
                row.child(
                    Label::new(preview)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .flex_1()
                        .min_w_0(),
                )
            })
            .child(
                Button::new("shelf-paste")
                    .label(t!("files.shelf.paste"))
                    .with_size(Size::Small)
                    .flex_shrink_0()
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(PasteItems), cx);
                    }),
            )
            .child(
                Button::new("shelf-clear")
                    .label(t!("files.shelf.clear"))
                    .with_size(Size::Small)
                    .flex_shrink_0()
                    .on_click(|_, _, cx| {
                        AppFileClipboard::clear(cx);
                    }),
            )
    }

    pub(super) fn render_shell_layout_row(
        &mut self,
        window: &mut Window,
        active_shell: Entity<ShellPanes>,
        show_info_pane: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sidebar_sections = self.sidebar_sections.clone();

        h_resizable("main-layout")
            .with_state(&window.use_keyed_state("main-layout", cx, |_, _| ResizableState::default()))
            .child(
                resizable_panel()
                    .size(px(240.))
                    .size_range(px(200.)..px(360.))
                    .fixed_size(true)
                    .flex_none()
                    .child(
                        div()
                            .id("sidebar-panel")
                            .size_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .child(render_sidebar(
                                cx.entity(),
                                self.active_navigation_target(cx),
                                &sidebar_sections,
                                window,
                                cx,
                            )),
                    ),
            )
            .child(
                resizable_panel().flex_1().min_w_0().child(
                    div()
                        .id("content-region")
                        .size_full()
                        .min_h_0()
                        .min_w_0()
                        .child(
                            h_resizable("main-with-info-pane")
                                .with_state(&window.use_keyed_state("main-with-info-pane", cx, |_, _| ResizableState::default()))
                                .child(resizable_panel().flex_1().min_w_0().child(
                                    self.render_content_column(window, active_shell, cx),
                                ))
                                .child(
                                    resizable_panel()
                                        .size(px(300.))
                                        .size_range(px(220.)..px(480.))
                                        .fixed_size(true)
                                        .flex_none()
                                        .visible(show_info_pane)
                                        .child(self.info_pane.clone()),
                                ),
                        ),
                ),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).target().clone();

        let (items, selected, hint) = match target {
            NavigationTarget::Path(_)
            | NavigationTarget::RecycleBin
            | NavigationTarget::FileTag(_) => {
                let b = pane.read(cx).file_browser().read(cx);
                let hint = match target {
                    NavigationTarget::RecycleBin => t!("main.status.recycle_bin").to_string(),
                    NavigationTarget::FileTag(_) => t!("main.status.file_tag").to_string(),
                    _ => t!("files.status.local").to_string(),
                };
                (b.item_count(), b.selected_count(), hint)
            }
            NavigationTarget::Home => (0, 0, t!("main.status.home").to_string()),
            NavigationTarget::Settings => (0, 0, t!("main.status.settings").to_string()),
        };

        let status_text = format!(
            "{} {}, {} {}",
            items,
            t!("files.status.items"),
            selected,
            t!("files.status.selected")
        );

        let jobs = TransferStatusGlobal::all_jobs(cx);
        let has_jobs = !jobs.is_empty();
        let active_count = jobs.iter().filter(|j| j.is_active()).count();

        let mut bar = h_flex()
            .id("status-bar")
            .flex_shrink_0()
            .h(px(32.))
            .px_3()
            .items_center()
            .justify_between()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border);

        bar = bar.child(
            Label::new(status_text)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        );

        if has_jobs {
            bar = bar.child(
                Button::new("status-center-toggle")
                    .label(if active_count > 0 {
                        format!("{} {}", active_count, t!("files.status_center.badge"))
                    } else {
                        t!("files.status_center.title").to_string()
                    })
                    .with_size(Size::Small)
                    .when(active_count > 0, |b| {
                        b.icon(crate::icons::compact_icon(gpui_component::IconName::ArrowUp))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_status_center = !this.show_status_center;
                        cx.notify();
                    })),
            );
        }

        bar.child(
            Label::new(hint)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
    }
}
