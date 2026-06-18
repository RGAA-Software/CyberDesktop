//! Files-style StatusCenter floating panel.

use gpui::{div, prelude::*, px, App, Hsla, IntoElement, SharedString, Styled, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    scroll::ScrollableElement as _,
    spinner::Spinner,
    v_flex, ActiveTheme as _, Disableable as _, Sizable as _, Size,
};
use rust_i18n::t;

use crate::app_state::{TransferJob, TransferJobStatus, TransferStatusGlobal};

/// Render the StatusCenter overlay panel.
///
/// `on_close` is called when the user presses Escape or clicks the close action.
pub fn render_status_center_panel(
    cx: &mut App,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let jobs = TransferStatusGlobal::all_jobs(cx);
    let has_any = !jobs.is_empty();
    let has_finished = TransferStatusGlobal::has_finished(cx);

    v_flex()
        .id("status-center-panel")
        .w(px(360.))
        .max_h(px(480.))
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .shadow_xl()
        .overflow_hidden()
        .child(render_header(has_finished, on_close, cx))
        .when(has_any, |this| {
            this.child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .gap_1()
                    .children(jobs.into_iter().map(|job| render_job_card(job, cx))),
            )
        })
        .when(!has_any, |this| {
            this.child(
                h_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .py_8()
                    .child(
                        gpui_component::label::Label::new(t!("files.status_center.empty"))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
        })
}

fn render_header(
    has_finished: bool,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
    cx: &mut App,
) -> impl IntoElement {
    h_flex()
        .id("status-center-header")
        .px_3()
        .py_2()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            gpui_component::label::Label::new(t!("files.status_center.title"))
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    Button::new("status-center-clear-completed")
                        .label(t!("files.status_center.clear_completed"))
                        .with_size(Size::Small)
                        .when(!has_finished, |this| this.disabled(true))
                        .on_click(|_, _, cx| {
                            TransferStatusGlobal::dismiss_completed(cx);
                        }),
                )
                .child(
                    Button::new("status-center-close")
                        .icon(gpui_component::IconName::Close)
                        .ghost()
                        .with_size(Size::Small)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            on_close(window, cx);
                        }),
                ),
        )
}

fn render_job_card(job: TransferJob, cx: &mut App) -> impl IntoElement {
    let is_active = job.is_active();
    let status = job.status();
    let job_id = job.id;

    let (icon_color, bg_color) = status_colors(status, cx);
    let status_text = match status {
        TransferJobStatus::Completed => t!("files.status.completed").to_string(),
        TransferJobStatus::Failed => t!("files.status.failed").to_string(),
        TransferJobStatus::Cancelled => t!("files.status.cancelled").to_string(),
        TransferJobStatus::Running => t!(
            "files.transfer.progress",
            completed = job.completed(),
            total = job.total
        )
        .to_string(),
    };

    v_flex()
        .id(format!("status-job-{}", job_id.0))
        .px_3()
        .py_2()
        .gap_2()
        .bg(cx.theme().background)
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    // Status icon circle
                    h_flex()
                        .w(px(28.))
                        .h(px(28.))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(bg_color)
                        .child(render_status_icon(status, icon_color)),
                )
                .child(
                    div().flex_1().min_w_0().overflow_hidden().child(
                        gpui_component::label::Label::new(job.message.clone())
                            .text_xs()
                            .text_color(if is_active {
                                cx.theme().accent_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .truncate(),
                    ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            gpui_component::label::Label::new(status_text.clone())
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(if is_active {
                            Button::new(format!("cancel-{}", job_id.0))
                                .label(t!("files.transfer.cancel"))
                                .with_size(Size::Small)
                                .flex_shrink_0()
                                .on_click(move |_, _, cx| {
                                    TransferStatusGlobal::request_cancel(job_id, cx);
                                })
                                .into_any_element()
                        } else {
                            Button::new(format!("dismiss-{}", job_id.0))
                                .icon(gpui_component::IconName::Close)
                                .ghost()
                                .with_size(Size::Small)
                                .flex_shrink_0()
                                .on_click(move |_, _, cx| {
                                    TransferStatusGlobal::dismiss(job_id, cx);
                                })
                                .into_any_element()
                        }),
                ),
        )
        .when(is_active, |this| {
            this.child(
                v_flex()
                    .gap_1()
                    .child(
                        Progress::new(format!("progress-{}", job_id.0))
                            .w_full()
                            .h(px(4.))
                            .value(job.fraction() * 100.),
                    )
                    .child(
                        gpui_component::label::Label::new(status_text)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
        })
}

fn render_status_icon(status: TransferJobStatus, icon_color: Hsla) -> impl IntoElement {
    match status {
        TransferJobStatus::Running => Spinner::new().small().color(icon_color).into_any_element(),
        _ => div()
            .child(
                gpui_component::label::Label::new(status_icon_glyph(status))
                    .text_sm()
                    .text_color(icon_color),
            )
            .into_any_element(),
    }
}

fn status_icon_glyph(status: TransferJobStatus) -> SharedString {
    match status {
        TransferJobStatus::Running => "↻".into(),
        TransferJobStatus::Completed => "✓".into(),
        TransferJobStatus::Failed => "!".into(),
        TransferJobStatus::Cancelled => "⊘".into(),
    }
}

fn status_colors(status: TransferJobStatus, cx: &App) -> (Hsla, Hsla) {
    match status {
        TransferJobStatus::Running => (cx.theme().primary, cx.theme().primary.opacity(0.15)),
        TransferJobStatus::Completed => {
            let green = gpui::hsla(142.0 / 360.0, 0.76, 0.36, 1.0);
            (green, gpui::hsla(142.0 / 360.0, 0.76, 0.36, 0.15))
        }
        TransferJobStatus::Failed => {
            let red = gpui::hsla(0.0, 0.84, 0.6, 1.0);
            (red, gpui::hsla(0.0, 0.84, 0.6, 0.15))
        }
        TransferJobStatus::Cancelled => {
            (cx.theme().muted_foreground, cx.theme().muted.opacity(0.15))
        }
    }
}
