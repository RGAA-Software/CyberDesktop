use cyberfiles_fs::{
    preview_kind, read_text_preview, FileItem, FileItemKind, PreviewKind,
};
use gpui::{img, prelude::*, ObjectFit, *};
use gpui_component::{
    alert::Alert,
    description_list::{DescriptionItem, DescriptionList},
    h_flex,
    label::Label,
    scroll::ScrollableElement as _,
    v_flex, ActiveTheme as _, IconName,
};
use rust_i18n::t;

use crate::icons::icon_foreground;
use crate::tab::{Tab, TabBar};

pub struct InfoPane {
    selected_tab: usize,
    item: Option<FileItem>,
}

impl InfoPane {
    pub fn new() -> Self {
        Self {
            selected_tab: 0,
            item: None,
        }
    }

    pub fn set_item(&mut self, item: Option<FileItem>) {
        self.item = item;
    }
}

impl Render for InfoPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_tab = self.selected_tab;
        let item = self.item.clone();

        v_flex()
            .id("info-pane")
            .size_full()
            .min_w_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                TabBar::new("info-pane-tabs")
                    .w_full()
                    .selected_index(selected_tab)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.selected_tab = *ix;
                        cx.notify();
                    }))
                    .child(Tab::new().label(t!("info_pane.tab.details")))
                    .child(Tab::new().label(t!("info_pane.tab.preview"))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .p_3()
                    .gap_3()
                    .child(tab_content(selected_tab, item.as_ref(), cx)),
            )
    }
}

fn tab_content(
    selected_tab: usize,
    item: Option<&FileItem>,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    if selected_tab == 0 {
        details_panel(item, cx).into_any_element()
    } else {
        preview_panel(item, cx).into_any_element()
    }
}

fn details_panel(item: Option<&FileItem>, cx: &mut Context<InfoPane>) -> impl IntoElement {
    let (name, detail_lines) = item_details(item);
    let path_label = t!("info_pane.path").to_string();
    let type_label = t!("info_pane.type").to_string();
    let modified_label = t!("info_pane.modified").to_string();

    v_flex()
        .w_full()
        .gap_3()
        .when_some(name.clone(), |panel, name| {
            panel
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_color(cx.theme().foreground)
                        .child(icon_foreground(IconName::Info, cx))
                        .child(Label::new(name).text_sm().text_color(cx.theme().foreground)),
                )
                .child(
                    DescriptionList::vertical()
                        .bordered(false)
                        .columns(1)
                        .children(detail_lines.into_iter().map(|(label, value)| {
                            let item = DescriptionItem::new(label.clone());
                            if label == path_label || label == type_label || label == modified_label
                            {
                                item.value(div().text_sm().child(value).into_any_element())
                            } else {
                                item.value(value)
                            }
                        })),
                )
        })
        .when(name.is_none(), |panel| {
            panel.child(Alert::info(
                "info-pane-empty",
                t!("info_pane.empty").to_string(),
            ))
        })
}

fn preview_panel(item: Option<&FileItem>, cx: &mut Context<InfoPane>) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .when(item.is_none(), |panel| panel.child(empty_preview()))
        .when_some(item.as_ref(), |panel, item| {
            if item.kind == FileItemKind::Folder {
                panel.child(Alert::info(
                    "info-pane-preview-folder",
                    t!("info_pane.preview.folder").to_string(),
                ))
            } else {
                match preview_kind(&item.path) {
                    Some(PreviewKind::Image | PreviewKind::Svg) => {
                        panel.child(preview_image_content(&item.path))
                    }
                    Some(kind @ (PreviewKind::Markdown | PreviewKind::Html | PreviewKind::Code | PreviewKind::Text)) => {
                        panel.child(preview_text_content(&item.path, kind, cx))
                    }
                    None => panel.child(Alert::warning(
                        "info-pane-preview-unsupported",
                        t!("info_pane.preview.unsupported").to_string(),
                    )),
                }
            }
        })
}

fn preview_image_content(path: &std::path::Path) -> impl IntoElement {
    img(path.to_path_buf())
        .w_full()
        .max_h(px(360.))
        .object_fit(ObjectFit::Contain)
}

fn preview_text_content(
    path: &std::path::Path,
    kind: PreviewKind,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    match read_text_preview(path) {
        Ok(text) => {
            let is_code_like = matches!(
                kind,
                PreviewKind::Code | PreviewKind::Html | PreviewKind::Markdown
            );
            v_flex()
                .w_full()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(icon_foreground(IconName::File, cx))
                        .child(
                            Label::new(preview_kind_title(kind))
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground),
                        )
                        .child(
                            Label::new(
                                path.extension()
                                    .and_then(|ext| ext.to_str())
                                    .map(|ext| format!(".{ext}"))
                                    .unwrap_or_default(),
                            )
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .max_h(px(420.))
                        .overflow_y_scrollbar()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .w_full()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .when(is_code_like, |this| {
                                    this.font_family(cx.theme().mono_font_family.clone())
                                })
                                .child(text),
                        ),
                )
                .into_any_element()
        }
        Err(error) => Alert::error(
            "info-pane-preview-error",
            format!("{}: {error}", t!("info_pane.preview.error")),
        )
        .into_any_element(),
    }
}

fn empty_preview() -> Alert {
    Alert::info(
        "info-pane-preview-empty",
        t!("info_pane.preview.empty").to_string(),
    )
}

fn preview_kind_title(kind: PreviewKind) -> &'static str {
    match kind {
        PreviewKind::Image => "Image preview",
        PreviewKind::Svg => "SVG preview",
        PreviewKind::Markdown => "Markdown preview",
        PreviewKind::Html => "HTML preview",
        PreviewKind::Code => "Code preview",
        PreviewKind::Text => "Text preview",
    }
}

fn item_details(item: Option<&FileItem>) -> (Option<String>, Vec<(String, String)>) {
    let Some(item) = item else {
        return (None, Vec::new());
    };

    let name = item.display_name.clone();
    let mut lines = vec![
        (
            t!("info_pane.path").to_string(),
            item.path.display().to_string(),
        ),
        (t!("info_pane.type").to_string(), item_type_label(item)),
    ];

    if let Some(size) = item.size {
        lines.push((t!("info_pane.size").to_string(), format_size(size)));
    }
    if let Some(modified) = item.modified {
        lines.push((
            t!("info_pane.modified").to_string(),
            format_system_time(modified),
        ));
    }

    (Some(name), lines)
}

fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|e| format!("{} file", e.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_system_time(time: std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};
    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}
