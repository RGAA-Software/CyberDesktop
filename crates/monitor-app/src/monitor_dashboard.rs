use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, relative, rgb, size,
    AnyElement, App, Axis, Context, ElementId, Entity, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    chart::AreaChart,
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::{ScrollableMask, Scrollbar, ScrollbarShow},
    v_flex, v_virtual_list, ActiveTheme, StyledExt, VirtualListScrollHandle,
};

use crate::monitor_actions::{
    CopyProcessInfo, RestartServiceAction, ResumeProcess, RevealProcessExe, RevealStartupItem,
    SetProcessAffinity, SetProcessIoPriority, SetProcessPriority, ShowProcessDetails,
    StartServiceAction, StopServiceAction, SuspendProcess, TerminateProcess, TerminateProcessTree,
};
use crate::monitor_icons;
use crate::cpu_platform::format_cpu_frequency_range;
use crate::monitor_model::{
    bytes_to_gb, chart_ticks, disk_key, disk_usage_percent, disk_used_gb, format_cpu_temperature,
    format_gpu_fan_speed, format_mem_size, format_network_link_speed, format_optional_frequency,
    format_tick, gpu_chart_color, gpu_chart_title, gpu_display_model, gpu_fan_meter_percent,
    gpu_key, gpu_memory_percent, latest_disk_rates, latest_network_rates, network_ipv4,
    network_key, sort_processes, MachineTelemetry, MonitorTab, ProcessSort, ProcessSortColumn,
    SortDirection,
};
use crate::sys_info::{SysProcessInfo, SysServiceInfo, SysStartupInfo, SysUserInfo};
use app_ui::{color_icon_box, ContextMenuExt};

/// Shared card container used across the dashboard. Callers should still apply
/// `.border_color(cx.theme().border)` and `.bg(cx.theme().secondary)` so the
/// theme is resolved at render time.
fn card(id: impl Into<SharedString>) -> Stateful<gpui::Div> {
    div().id(id.into()).gap_3().p_3().rounded_md().border_1()
}

pub fn topbar_icon_button(
    id: impl Into<ElementId>,
    icon_path: &'static str,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let hover_bg = topbar_hover_bg(cx);
    let strong_border = border_strong(cx);
    div()
        .id(id)
        .w(px(34.))
        .h(px(34.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .text_color(cx.theme().foreground)
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(hover_bg)
                .border_color(strong_border)
                .text_color(cx.theme().primary)
        })
        .child(monitor_icons::topbar_icon(icon_path))
}

/// Equal-width KPI row (design: 3/4/6 column grids without wrapping).
fn metric_grid_row_equal() -> gpui::Div {
    h_flex().gap(px(14.)).w_full()
}

fn striped_row_bg(index: usize, cx: &App) -> Option<Hsla> {
    if index % 2 == 1 {
        Some(cx.theme().primary.opacity(0.03))
    } else {
        None
    }
}

const PROCESS_COL_PID: Pixels = px(80.);
const PROCESS_COL_NAME: Pixels = px(160.);
const PROCESS_COL_STATUS: Pixels = px(80.);
const PROCESS_COL_CPU: Pixels = px(80.);
const PROCESS_COL_MEM: Pixels = px(100.);
const PROCESS_COL_VMEM: Pixels = px(120.);
const PROCESS_COL_READ: Pixels = px(100.);
const PROCESS_COL_WRITE: Pixels = px(100.);
const PROCESS_COL_GPU: Pixels = px(180.);
const PROCESS_COL_GPU_USAGE: Pixels = px(80.);
const PROCESS_COL_GPU_MEM: Pixels = px(110.);
const PROCESS_COL_CMD: Pixels = px(800.);

fn process_table_min_width() -> Pixels {
    px(1620. + 180. + 80. + 110. + 64. + 48.)
}

/// Extra space between right-aligned numeric cells and following left-aligned text.
const PROCESS_COL_INNER_PAD: Pixels = px(10.);
const PROCESS_COL_GAP: Pixels = px(12.);

const SERVICE_COL_NAME: Pixels = px(250.);
const SERVICE_COL_DISPLAY: Pixels = px(400.);
const SERVICE_COL_STATUS: Pixels = px(100.);
const SERVICE_COL_START: Pixels = px(100.);

fn service_table_min_width() -> Pixels {
    px(250. + 400. + 100. + 100. + 64.)
}

const STARTUP_COL_NAME: Pixels = px(180.);
const STARTUP_COL_COMMAND: Pixels = px(600.);
const STARTUP_COL_LOCATION: Pixels = px(240.);

fn startup_table_min_width() -> Pixels {
    px(180. + 600. + 240. + 64.)
}

const USER_COL_NAME: Pixels = px(200.);
const USER_COL_UID: Pixels = px(400.);
const USER_COL_GID: Pixels = px(120.);
const USER_COL_GROUPS: Pixels = px(300.);

fn user_table_min_width() -> Pixels {
    px(200. + 400. + 120. + 300. + 64.)
}

pub fn tab_manages_bottom_padding(tab: MonitorTab) -> bool {
    matches!(
        tab,
        MonitorTab::Processes | MonitorTab::Services | MonitorTab::Startup | MonitorTab::Users
    )
}

/// Design token `--panel-2` (#f7f9fc / #0f1521).
fn panel_2(cx: &App) -> Hsla {
    cx.theme().secondary_active
}

/// Design token `--line-strong` (#c9d2e4 / #33405c).
fn border_strong(cx: &App) -> Hsla {
    if cx.theme().mode.is_dark() {
        rgb(0x33405c).into()
    } else {
        rgb(0xc9d2e4).into()
    }
}

/// Design token `--hover` for topbar icon buttons (#eef3fb / #171d2c).
fn topbar_hover_bg(cx: &App) -> Hsla {
    if cx.theme().mode.is_dark() {
        rgb(0x171d2c).into()
    } else {
        rgb(0xeef3fb).into()
    }
}

fn chart_title_dot(color: Hsla) -> gpui::Div {
    div()
        .relative()
        .w(px(16.))
        .h(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .absolute()
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .bg(color.opacity(0.12)),
        )
        .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(color))
}

fn monitor_search_input(search: &Entity<InputState>) -> Input {
    Input::new(search)
        .w(px(280.))
        .h(px(38.))
        .rounded(px(7.))
}

/// Page title in the content topbar (design: 20px semibold).
pub fn monitor_page_title_label(title: impl Into<SharedString>) -> Label {
    Label::new(title).text_size(px(20.)).font_semibold()
}

/// Title + subtitle block vertically centered in the 62px topbar.
pub fn monitor_title_crumb<V>(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &Context<V>,
) -> impl IntoElement {
    div()
        .h_full()
        .flex()
        .flex_col()
        .justify_center()
        .child(
            monitor_page_title_label(title)
                .line_height(px(24.))
                .text_color(cx.theme().foreground),
        )
        .child(
            Label::new(subtitle)
                .text_xs()
                .line_height(px(16.))
                .mt(px(4.))
                .text_color(cx.theme().muted_foreground),
        )
}

pub const MONITOR_MAIN_TITLE_BAR_HEIGHT: Pixels = px(62.);

pub fn render_monitor_brand<V>(cx: &Context<V>) -> impl IntoElement {
    h_flex()
        .h(MONITOR_MAIN_TITLE_BAR_HEIGHT)
        .w_full()
        .gap(px(12.))
        .items_center()
        .pl(px(22.))
        .pr(px(22.))
        .border_b_1()
        .border_color(cx.theme().border)
        .child(color_icon_box(monitor_icons::APP_LOGO_PATH, px(38.)))
        .child(
            v_flex()
                .justify_center()
                .child(
                    Label::new("CyberMonitor")
                        .text_base()
                        .font_semibold()
                        .text_color(cx.theme().foreground),
                )
                .child(
                    Label::new("SYSTEM INSIGHT CONSOLE")
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
}

pub fn render_monitor_client_sidebar<V, F>(
    active_tab: MonitorTab,
    on_click: F,
    cx: &Context<V>,
) -> impl IntoElement
where
    F: Fn(MonitorTab, &mut Window, &mut App) + Clone + 'static,
{
    v_flex()
        .id("monitor-sidebar")
        .w(px(248.))
        .h_full()
        .border_r_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .child(render_monitor_brand(cx))
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .gap(px(18.))
                .px(px(14.))
                .pt(px(16.))
                .pb(px(16.))
                .child(render_monitor_nav(active_tab, on_click, cx)),
        )
}

pub fn render_dashboard<V: Render, F>(
    telemetry: &MachineTelemetry,
    active_tab: MonitorTab,
    process_scroll_handle: &VirtualListScrollHandle,
    process_h_scroll_handle: &VirtualListScrollHandle,
    process_search: &Entity<InputState>,
    process_sort: ProcessSort,
    service_scroll_handle: &VirtualListScrollHandle,
    service_h_scroll_handle: &VirtualListScrollHandle,
    service_search: &Entity<InputState>,
    startup_scroll_handle: &VirtualListScrollHandle,
    startup_h_scroll_handle: &VirtualListScrollHandle,
    startup_search: &Entity<InputState>,
    user_scroll_handle: &VirtualListScrollHandle,
    user_h_scroll_handle: &VirtualListScrollHandle,
    user_search: &Entity<InputState>,
    on_cycle_sort: F,
    window: &mut Window,
    cx: &mut Context<V>,
) -> AnyElement
where
    F: Fn(ProcessSortColumn, &mut Window, &mut App) + Clone + 'static,
{
    match active_tab {
        MonitorTab::Overview => render_overview_tab(telemetry, cx).into_any_element(),
        MonitorTab::Cpu => render_cpu_tab(telemetry, cx).into_any_element(),
        MonitorTab::Memory => render_memory_tab(telemetry, cx).into_any_element(),
        MonitorTab::Gpu => render_gpu_tab(telemetry, cx).into_any_element(),
        MonitorTab::Storage => render_storage_tab(telemetry, cx).into_any_element(),
        MonitorTab::Network => render_network_tab(telemetry, cx).into_any_element(),
        MonitorTab::Processes => render_processes_tab(
            telemetry,
            process_scroll_handle,
            process_h_scroll_handle,
            process_search,
            process_sort,
            on_cycle_sort,
            window,
            cx,
        )
        .into_any_element(),
        MonitorTab::Services => render_services_tab(
            telemetry,
            service_scroll_handle,
            service_h_scroll_handle,
            service_search,
            cx,
        )
        .into_any_element(),
        MonitorTab::Startup => render_startup_tab(
            telemetry,
            startup_scroll_handle,
            startup_h_scroll_handle,
            startup_search,
            cx,
        )
        .into_any_element(),
        MonitorTab::Users => render_users_tab(
            telemetry,
            user_scroll_handle,
            user_h_scroll_handle,
            user_search,
            cx,
        )
        .into_any_element(),
    }
}

pub fn render_connection_summary<V>(details: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .px_4()
        .py_2()
        .bg(cx.theme().secondary)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Label::new(details.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
}

pub fn render_monitor_nav<V, F>(
    active_tab: MonitorTab,
    on_click: F,
    cx: &Context<V>,
) -> impl IntoElement
where
    F: Fn(MonitorTab, &mut Window, &mut App) + Clone + 'static,
{
    let tabs: [(MonitorTab, &'static str, &'static str); 10] = [
        (MonitorTab::Overview, "总览", monitor_icons::OVERVIEW),
        (MonitorTab::Cpu, "CPU", monitor_icons::CPU),
        (MonitorTab::Memory, "内存", monitor_icons::MEMORY),
        (MonitorTab::Gpu, "GPU", monitor_icons::GPU),
        (MonitorTab::Storage, "存储", monitor_icons::STORAGE),
        (MonitorTab::Network, "网络", monitor_icons::NETWORK),
        (MonitorTab::Processes, "进程", monitor_icons::PROCESS),
        (MonitorTab::Services, "服务", monitor_icons::SERVICE),
        (MonitorTab::Startup, "启动项", monitor_icons::STARTUP),
        (MonitorTab::Users, "用户", monitor_icons::USERS),
    ];
    let hover_bg = if cx.theme().mode.is_dark() {
        rgb(0x171d2c)
    } else {
        rgb(0xeef3fb)
    };
    let strong_border = border_strong(cx);

    v_flex()
        .id("monitor-nav")
        .gap(px(7.))
        .children(tabs.iter().map(|(tab, label, path)| {
            let tab = *tab;
            let on_click = on_click.clone();
            let is_active = active_tab == tab;
            let icon_box_bg = if is_active {
                cx.theme().primary.opacity(0.10)
            } else {
                cx.theme().transparent
            };

            div()
                .id(format!("nav-{label}"))
                .h(px(44.))
                .px(px(12.))
                .rounded(px(8.))
                .cursor_pointer()
                .border_1()
                .text_color(if is_active {
                    cx.theme().primary
                } else {
                    cx.theme().foreground
                })
                .border_color(if is_active {
                    strong_border
                } else {
                    cx.theme().transparent
                })
                .bg(if is_active {
                    cx.theme().secondary
                } else {
                    cx.theme().transparent
                })
                .when(is_active, |this| this.shadow_md())
                .when(!is_active, |this| {
                    this.hover(|style| {
                        style
                            .bg(hover_bg)
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().primary)
                    })
                })
                .relative()
                .when(is_active, |this| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(-14.))
                            .top(px(9.))
                            .bottom(px(9.))
                            .w(px(3.))
                            .rounded_full()
                            .bg(cx.theme().primary),
                    )
                })
                .child(
                    h_flex()
                        .h_full()
                        .gap(px(12.))
                        .items_center()
                        .child(
                            div()
                                .w(px(22.))
                                .h(px(22.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .bg(icon_box_bg)
                                .when(!is_active, |this| {
                                    this.hover(|style| {
                                        style.bg(cx.theme().primary.opacity(0.10))
                                    })
                                })
                                .child(monitor_icons::nav_icon(path)),
                        )
                        .child(Label::new(*label).text_sm()),
                )
                .on_click(move |_, window, cx| {
                    on_click(tab, window, cx);
                })
        }))
}

fn render_metric_card<V>(
    id: &str,
    title: &str,
    value: String,
    percent: Option<f32>,
    progress_color: Option<Hsla>,
    cx: &Context<V>,
) -> impl IntoElement {
    render_metric_card_with_height(id, title, value, percent, progress_color, px(96.), false, false, cx)
}

fn render_overview_metric_card<V>(
    id: &str,
    title: &str,
    value: String,
    percent: Option<f32>,
    progress_color: Option<Hsla>,
    cx: &Context<V>,
) -> impl IntoElement {
    render_metric_card_with_height(id, title, value, percent, progress_color, px(96.), true, false, cx)
}

fn render_process_header_metric_card<V>(
    id: &str,
    title: &str,
    value: String,
    cx: &Context<V>,
) -> impl IntoElement {
    render_metric_card_with_height(id, title, value, None, None, px(96.), true, true, cx)
}

fn render_metric_card_with_height<V>(
    id: &str,
    title: &str,
    value: String,
    percent: Option<f32>,
    progress_color: Option<Hsla>,
    height: Pixels,
    overview_value: bool,
    large_overview_value: bool,
    cx: &Context<V>,
) -> impl IntoElement {
    let value_is_long = value.len() > 14;
    let show_progress = percent.is_some();
    let progress_percent = percent.unwrap_or(0.0).clamp(0.0, 100.0);
    let progress_color = progress_color.unwrap_or(cx.theme().primary);

    card(id)
        .flex_1()
        .min_w(px(0.))
        .min_h_0()
        .h(height)
        .p(px(14.))
        .gap_2()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            Label::new(title.to_uppercase())
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .line_height(px(14.))
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .gap_1()
                .when(overview_value, |this| this.mt(px(10.)))
                .child(if overview_value {
                    div()
                        .w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            Label::new(value)
                                .when(large_overview_value, |this| {
                                    this.text_base().line_height(px(22.))
                                })
                                .when(!large_overview_value, |this| {
                                    this.text_sm().line_height(px(20.))
                                })
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .text_color(cx.theme().foreground),
                        )
                        .into_any_element()
                } else {
                    Label::new(value)
                        .when(value_is_long, |this| this.text_sm())
                        .when(!value_is_long, |this| this.text_2xl())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(value_is_long.then_some(20.).unwrap_or(28.)))
                        .text_color(cx.theme().foreground)
                        .into_any_element()
                })
                .when(show_progress, |this| {
                    this.child(
                        div()
                            .id(format!("{id}-bar"))
                            .w_full()
                            .h(px(6.))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(cx.theme().primary.opacity(0.10))
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(progress_color)
                                    .w(relative(progress_percent / 100.0)),
                            ),
                    )
                }),
        )
}

fn render_chart<V, T: Clone + 'static>(
    id: &str,
    title: &str,
    data: Vec<T>,
    x_fn: impl Fn(&T) -> String + 'static,
    y_fn: impl Fn(&T) -> f64 + 'static,
    color: Hsla,
    unit: &str,
    x_unit: &str,
    y_max: Option<f64>,
    show_x_axis: bool,
    show_y_ticks: bool,
    cx: &Context<V>,
) -> impl IntoElement {
    let current_value = data.last().map(&y_fn).unwrap_or(0.0);
    let max_value = data
        .iter()
        .map(&y_fn)
        .fold(0.0_f64, f64::max)
        .max(y_max.unwrap_or(0.0))
        .max(1.0);
    let tick_values = chart_ticks(max_value);
    let compact = !show_x_axis && !show_y_ticks;
    let chart_body_h = if compact { px(120.) } else { px(220.) };
    let plot_bg = panel_2(cx);
    let x_label_count = if show_x_axis { 5 } else { 0 };
    let x_labels: Vec<SharedString> = if show_x_axis && x_label_count > 1 {
        let n = data.len().max(1);
        (0..x_label_count)
            .map(|i| {
                let idx = ((n - 1) as f32 * i as f32 / (x_label_count - 1) as f32).round() as usize;
                x_fn(&data[idx.min(n - 1)]).into()
            })
            .collect()
    } else {
        vec![]
    };
    let x_tick_margin = if show_x_axis {
        data.len().saturating_add(1)
    } else {
        1
    };
    let mut chart = AreaChart::new(data)
        .x(x_fn)
        .y(y_fn)
        .linear()
        .stroke(color)
        .fill(linear_gradient(
            0.,
            linear_color_stop(color.opacity(0.40), 1.),
            linear_color_stop(plot_bg.opacity(0.05), 0.),
        ))
        .tick_margin(x_tick_margin)
        .x_axis(show_x_axis);
    if let Some(y_max) = y_max {
        chart = chart
            .y(move |_| y_max)
            .stroke(color.opacity(0.0))
            .fill(plot_bg.opacity(0.0));
    }

    v_flex()
        .id(SharedString::from(id.to_string()))
        .gap_3()
        .pl(px(16.))
        .pr(px(16.))
        .pt(px(14.))
        .pb(px(16.))
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .pb(px(10.))
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(chart_title_dot(color))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground)
                                .child(title.to_string()),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("当前 {:.1} {}", current_value, unit)),
                ),
        )
        .child(
            h_flex()
                .gap_3()
                .flex_1()
                .when(show_y_ticks, |this| {
                    this.child(
                        v_flex()
                            .w(px(52.))
                            .h_full()
                            .justify_between()
                            .items_end()
                            .pt_3()
                            .pb_8()
                            .pr_1()
                            .children(tick_values.iter().map(|value| {
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format_tick(*value, unit))
                            })),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .h(chart_body_h)
                        .rounded(px(6.))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(plot_bg)
                        .p(px(10.))
                        .relative()
                        .child(chart)
                        .when(show_x_axis && !x_labels.is_empty(), |this| {
                        this.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .h(px(18.))
                                .child(
                                    h_flex()
                                        .w_full()
                                        .h_full()
                                        .items_end()
                                        .px(px(10.))
                                        .justify_between()
                                        .children(x_labels.iter().map(|text| {
                                            Label::new(text.clone())
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                        })),
                                ),
                        )
                    },
                )),
        )
        .when(false, |this| {
            this.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Y: {unit}")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("X: {x_unit} ->")),
                    ),
            )
        })
}

fn render_overview_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let send_rate = telemetry.latest_send_rate();
    let recv_rate = telemetry.latest_recv_rate();
    let send_percent = ((send_rate / 10.0) * 100.0).min(100.0) as f32;
    let recv_percent = ((recv_rate / 10.0) * 100.0).min(100.0) as f32;
    let primary_gpu = telemetry.current.gpus.first();
    let cpu_trend_title = format!("CPU 趋势 ({})", telemetry.current.cpu.brand.trim());
    let gpu_rows: Vec<_> = telemetry
        .current
        .gpus
        .iter()
        .take(16)
        .enumerate()
        .filter_map(|(index, gpu)| {
            let gpu_id = gpu_key(gpu);
            let history = telemetry.gpu_history.get(&gpu_id)?;
            if history.is_empty() {
                return None;
            }
            let gpu_history: Vec<_> = history.iter().cloned().collect();
            let gpu_mem_total = gpu.mem_total_gb as f64;
            let gpu_mem_total = if gpu_mem_total > 0.0 {
                gpu_mem_total
            } else {
                1.0
            };
            Some((index, gpu.clone(), gpu_history, gpu_mem_total))
        })
        .collect();

    v_flex()
        .gap(px(14.))
        .child(
            metric_grid_row_equal()
                .child(render_overview_metric_card(
                    "overview-cpu",
                    "CPU 使用率",
                    format!("{:.1}%", telemetry.latest_cpu_percent()),
                    Some(telemetry.latest_cpu_percent()),
                    Some(cx.theme().red),
                    cx,
                ))
                .child(render_overview_metric_card(
                    "overview-mem",
                    "内存使用",
                    format!(
                        "{:.1} / {:.1} GB",
                        bytes_to_gb(telemetry.current.mem.used),
                        bytes_to_gb(telemetry.current.mem.total)
                    ),
                    Some(telemetry.latest_mem_percent()),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_overview_metric_card(
                    "overview-disk",
                    "系统盘占用",
                    telemetry.system_disk_label(),
                    Some(telemetry.system_disk_percent()),
                    Some(cx.theme().yellow),
                    cx,
                ))
                .child(render_overview_metric_card(
                    "overview-gpu",
                    "首块 GPU",
                    telemetry
                        .current
                        .gpus
                        .first()
                        .map(|gpu| gpu_display_model(&gpu.brand))
                        .unwrap_or_else(|| "无 GPU 数据".to_string()),
                    telemetry
                        .current
                        .gpus
                        .first()
                        .map(|gpu| gpu.gpu_utilization as f32),
                    primary_gpu.map(|_| gpu_chart_color(0)).or(Some(cx.theme().primary)),
                    cx,
                ))
                .child(render_overview_metric_card(
                    "overview-send",
                    "主网卡发送速率",
                    format!("{:.2} MB/s", send_rate),
                    Some(send_percent),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_overview_metric_card(
                    "overview-recv",
                    "主网卡接收速率",
                    format!("{:.2} MB/s", recv_rate),
                    Some(recv_percent),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            v_flex()
                .gap(px(14.))
                .child(
                    h_flex()
                        .gap(px(14.))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            "overview-chart-cpu",
                            &cpu_trend_title,
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.cpu_usage,
                            cx.theme().red,
                            "%",
                            "时间",
                            Some(100.0),
                            true,
                            true,
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            "overview-chart-mem",
                            "内存占用趋势",
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.mem_usage_percent,
                            cx.theme().primary,
                            "%",
                            "时间",
                            Some(100.0),
                            true,
                            true,
                            cx,
                        ))),
                )
                .children(gpu_rows.into_iter().map(|(index, gpu, gpu_history, gpu_mem_total)| {
                    let color = gpu_chart_color(index);
                    let usage_title = gpu_chart_title("GPU 使用率", &gpu);
                    let mem_title = gpu_chart_title("显存占用率", &gpu);
                    h_flex()
                        .gap(px(14.))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            &format!("overview-chart-gpu-usage-{index}"),
                            &usage_title,
                            gpu_history.clone(),
                            |point| point.time.clone(),
                            |point| point.usage,
                            color,
                            "%",
                            "时间",
                            Some(100.0),
                            true,
                            true,
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            &format!("overview-chart-gpu-mem-{index}"),
                            &mem_title,
                            gpu_history,
                            |point| point.time.clone(),
                            move |point| {
                                (point.memory_used_gb / gpu_mem_total * 100.0).clamp(0.0, 100.0)
                            },
                            color,
                            "%",
                            "时间",
                            Some(100.0),
                            true,
                            true,
                            cx,
                        )))
                }))
                .child(
                    h_flex()
                        .gap(px(14.))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            "overview-chart-send",
                            "网络发送速率",
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.net_send_mb,
                            cx.theme().primary,
                            "MB/s",
                            "时间",
                            None,
                            true,
                            true,
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(render_chart(
                            "overview-chart-recv",
                            "网络接收速率",
                            history,
                            |point| point.time.clone(),
                            |point| point.net_recv_mb,
                            cx.theme().yellow,
                            "MB/s",
                            "时间",
                            None,
                            true,
                            true,
                            cx,
                        ))),
                ),
        )
        .child(div().h(px(10.)))
}

fn render_cpu_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let cpu_usage = telemetry.current.cpu.usage;
    let core_count = telemetry.current.cpu.cpus.len();
    let freq_y_max = telemetry.current.cpu.max_frequency.max(0.1) as f64;

    v_flex()
        .gap(px(14.))
        .child(
            h_flex()
                .gap(px(14.))
                .items_stretch()
                .child(
                    card("cpu-usage")
                        .w(px(280.))
                        .flex_none()
                        .h_full()
                        .flex()
                        .flex_col()
                        .items_start()
                        .p(px(16.))
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            v_flex()
                                .gap(px(10.))
                                .child(inline_stat_row(
                                    "当前使用率",
                                    &format!("{cpu_usage:.1}%"),
                                    cx,
                                ))
                                .child(inline_stat_row("逻辑核心", &core_count.to_string(), cx))
                                .child(inline_stat_row(
                                    "当前频率",
                                    &format_optional_frequency(telemetry.current.cpu.current_frequency),
                                    cx,
                                ))
                                .child(inline_stat_row(
                                    "最大频率",
                                    &format!("{:.2} GHz", telemetry.current.cpu.max_frequency),
                                    cx,
                                ))
                                .child(inline_stat_row(
                                    "Threads",
                                    &telemetry.current.thread_count.to_string(),
                                    cx,
                                ))
                                .child(inline_stat_row(
                                    "Handles",
                                    &telemetry.current.handle_count.to_string(),
                                    cx,
                                ))
                                .child(inline_stat_row("Up time", &telemetry.current.uptime, cx))
                                .child(inline_stat_row(
                                    "Processes",
                                    &telemetry.current.processes.len().to_string(),
                                    cx,
                                )),
                        )
                )
                .child(
                    card("cpu-info")
                        .flex_1()
                        .min_w(px(320.))
                        .h_full()
                        .p(px(16.))
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            Label::new("基本信息".to_uppercase())
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            v_flex()
                                .mt(px(14.))
                                .gap(px(16.))
                                .child(
                                    h_flex()
                                        .gap(px(16.))
                                        .items_stretch()
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "型号",
                                            &telemetry.current.cpu.brand,
                                            cx,
                                        )))
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "硬件虚拟化",
                                            &telemetry.current.cpu.virtualization,
                                            cx,
                                        )))
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "架构",
                                            &format!(
                                                "x64 / {} Cores / {} Threads",
                                                telemetry.current.cpu.physical_cores.max(1),
                                                core_count
                                            ),
                                            cx,
                                        ))),
                                )
                                .child(
                                    h_flex()
                                        .gap(px(16.))
                                        .items_stretch()
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "基准/最大频率",
                                            &format_cpu_frequency_range(
                                                telemetry.current.cpu.base_frequency,
                                                telemetry.current.cpu.max_frequency,
                                            ),
                                            cx,
                                        )))
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "L1/L2/L3缓存",
                                            &telemetry.current.cpu.cache_summary,
                                            cx,
                                        )))
                                        .child(div().flex_1().min_w_0().child(info_item(
                                            "当前温度",
                                            &format_cpu_temperature(
                                                &telemetry.current.cpu,
                                                &telemetry.current.components,
                                            ),
                                            cx,
                                        ))),
                                ),
                        ),
                ),
        )
        .child(
            h_flex()
                .gap(px(14.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(render_chart(
                            "cpu-chart",
                            "CPU 总使用率",
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.cpu_usage,
                            cx.theme().red,
                            "%",
                            "时间",
                            Some(100.0),
                            true,
                            true,
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(render_chart(
                            "cpu-freq-chart",
                            "CPU 频率",
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.cpu_frequency,
                            cx.theme().primary,
                            "GHz",
                            "时间",
                            Some(freq_y_max),
                            true,
                            true,
                            cx,
                        )),
                ),
        )
        .children(
            (0..telemetry.current.cpu.cpus.len())
                .step_by(5)
                .map(|start| {
                    h_flex().gap(px(14.)).children((0..5).map(|offset| {
                        let index = start + offset;
                        if index >= telemetry.current.cpu.cpus.len() {
                            return div().flex_1().min_h(px(160.)).into_any_element();
                        }
                        let core_history: Vec<_> = history
                            .iter()
                            .map(|point| {
                                (
                                    point.time.clone(),
                                    point.cpu_cores.get(index).copied().unwrap_or(0.0),
                                )
                            })
                            .collect();
                        div()
                            .flex_1()
                            .min_w(px(180.))
                            .min_h(px(160.))
                            .child(render_chart(
                                &format!("cpu-core-{index}-chart"),
                                &format!("Core {}", index + 1),
                                core_history,
                                |point| point.0.clone(),
                                |point| point.1,
                                cx.theme().red,
                                "%",
                                "时间",
                                Some(100.0),
                                false,
                                false,
                                cx,
                            ))
                            .into_any_element()
                    }))
                }),
        )
        .child(div().h(px(10.)))
}

fn metric_item<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .p(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(panel_2(cx))
        .child(
            Label::new(label.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div().mt(px(6.)).child(
                Label::new(value.to_string())
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .line_height(px(20.))
                    .text_color(cx.theme().foreground),
            ),
        )
}

fn render_memory_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let mem_percent = telemetry.latest_mem_percent();

    v_flex()
        .gap(px(14.))
        .child(
            card("memory-hero")
                .p(px(16.))
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(
                    Label::new("内存概览".to_uppercase())
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    h_flex()
                        .mt(px(12.))
                        .gap(px(14.))
                        .child(div().flex_1().min_w_0().child(metric_item(
                            "总内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.total)),
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(metric_item(
                            "已用内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.used)),
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(metric_item(
                            "可用内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.available)),
                            cx,
                        )))
                        .child(div().flex_1().min_w_0().child(metric_item(
                            "使用率",
                            &format!("{:.1}%", mem_percent),
                            cx,
                        ))),
                ),
        )
        .child(
            h_flex()
                .gap(px(14.))
                .child(div().flex_1().min_w_0().child(render_chart(
                    "mem-chart",
                    "内存已用",
                    history.clone(),
                    |point| point.time.clone(),
                    |point| point.mem_used_gb,
                    cx.theme().primary,
                    "GB",
                    "时间",
                    Some(bytes_to_gb(telemetry.current.mem.total)),
                    true,
                    true,
                    cx,
                )))
                .child(div().flex_1().min_w_0().child(render_chart(
                    "mem-usage-chart",
                    "内存使用率",
                    history,
                    |point| point.time.clone(),
                    |point| point.mem_usage_percent,
                    cx.theme().primary,
                    "%",
                    "时间",
                    Some(100.0),
                    true,
                    true,
                    cx,
                ))),
        )
        .child(render_memory_advanced_metrics(telemetry, cx))
}

fn render_memory_advanced_metrics<V>(
    telemetry: &MachineTelemetry,
    cx: &Context<V>,
) -> impl IntoElement {
    let mem = &telemetry.current.mem;
    let has_advanced = mem.committed > 0
        || mem.system_cache > 0
        || mem.kernel_ws > 0
        || mem.swap_total > 0;

    v_flex()
        .gap(px(14.))
        .when(has_advanced, |this| {
            this.child(
                h_flex()
                    .gap(px(14.))
                    .items_stretch()
                    .child(
                        card("memory-virtual")
                            .flex_1()
                            .min_w_0()
                            .p(px(16.))
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(
                                Label::new("虚拟内存".to_uppercase())
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                h_flex()
                                    .mt(px(12.))
                                    .gap(px(14.))
                                    .items_stretch()
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "提交大小",
                                        &format_mem_size(mem.committed),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "峰值提交",
                                        &format_mem_size(mem.commit_peak),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "提交限制",
                                        &format_mem_size(mem.commit_limit),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "交换区已用",
                                        &format_mem_size(mem.swap_used),
                                        cx,
                                    ))),
                            ),
                    )
                    .child(
                        card("memory-physical-advanced")
                            .flex_1()
                            .min_w_0()
                            .p(px(16.))
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(
                                Label::new("物理内存".to_uppercase())
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                h_flex()
                                    .mt(px(12.))
                                    .gap(px(14.))
                                    .items_stretch()
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "硬件保留",
                                        &format_mem_size(mem.hw_reserved),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "缓存 WS",
                                        &format_mem_size(mem.system_cache),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "内核 WS",
                                        &format_mem_size(mem.kernel_ws),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(metric_item(
                                        "分页/非分页池",
                                        &format!(
                                            "{} / {}",
                                            format_mem_size(mem.kernel_paged),
                                            format_mem_size(mem.kernel_nonpaged)
                                        ),
                                        cx,
                                    ))),
                            ),
                    ),
            )
        })
        .when(!has_advanced, |this| {
            this.child(empty_state(
                "高级内存指标需要 Windows 系统 API（GetPerformanceInfo）",
                cx,
            ))
        })
}

fn chip<V>(label: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .h(px(24.))
        .px(px(10.))
        .rounded_full()
        .border_1()
        .border_color(cx.theme().primary.opacity(0.18))
        .bg(cx.theme().primary.opacity(0.10))
        .flex()
        .items_center()
        .child(
            Label::new(label.to_string())
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().primary),
        )
}

fn render_gpu_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap(px(14.))
        .when(telemetry.current.gpus.is_empty(), |this| {
            this.child(empty_state("当前没有 GPU 监控数据", cx))
        })
        .children(telemetry.current.gpus.iter().map(|gpu| {
            let gpu_id = gpu_key(gpu);
            let history = telemetry
                .gpu_history
                .get(&gpu_id)
                .map(|items| items.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let power_w = gpu.power_limit as f64 / 1000.0;

            card(format!("gpu-panel-{gpu_id}"))
                .p(px(16.))
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(
                    v_flex()
                        .gap(px(10.))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    Label::new(gpu.brand.clone())
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().foreground),
                                )
                                .child(chip("独立显卡 · 在线", cx)),
                        )
                        .child(
                            metric_grid_row_equal()
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-usage"),
                                    "利用率",
                                    format!("{}%", gpu.gpu_utilization),
                                    Some(gpu.gpu_utilization as f32),
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-temp"),
                                    "温度",
                                    format!("{} °C", gpu.temperature),
                                    Some((gpu.temperature as f32).clamp(0.0, 100.0)),
                                    Some(cx.theme().red),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-fan"),
                                    "风扇转速",
                                    format_gpu_fan_speed(gpu),
                                    gpu_fan_meter_percent(gpu),
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-vram"),
                                    "显存",
                                    format!("{:.2} / {:.2} GB", gpu.mem_used_gb, gpu.mem_total_gb),
                                    Some(gpu_memory_percent(gpu)),
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-encoder"),
                                    "编码器",
                                    format!("{}%", gpu.encoder_utilization),
                                    Some(gpu.encoder_utilization as f32),
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("gpu-{gpu_id}-power"),
                                    "功耗上限",
                                    format!("{:.1} W", power_w),
                                    Some(((power_w / 300.0) * 100.0).clamp(0.0, 100.0) as f32),
                                    Some(cx.theme().primary),
                                    cx,
                                )),
                        )
                        .child(
                            h_flex()
                                .gap(px(14.))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("gpu-{gpu_id}-chart-usage"),
                                    "GPU 利用率",
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.usage,
                                    cx.theme().primary,
                                    "%",
                                    "时间",
                                    Some(100.0),
                                    true,
                                    true,
                                    cx,
                                )))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("gpu-{gpu_id}-chart-temp"),
                                    "GPU 温度",
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.temperature,
                                    cx.theme().red,
                                    "°C",
                                    "时间",
                                    Some(100.0),
                                    true,
                                    true,
                                    cx,
                                ))),
                        )
                        .child(
                            h_flex()
                                .gap(px(14.))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("gpu-{gpu_id}-chart-mem"),
                                    "显存占用",
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.memory_used_gb,
                                    cx.theme().primary,
                                    "GB",
                                    "时间",
                                    Some(gpu.mem_total_gb as f64),
                                    true,
                                    true,
                                    cx,
                                )))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("gpu-{gpu_id}-chart-decoder"),
                                    "解码器使用率",
                                    history,
                                    |point| point.time.clone(),
                                    |point| point.decoder_usage,
                                    cx.theme().primary,
                                    "%",
                                    "时间",
                                    Some(100.0),
                                    true,
                                    true,
                                    cx,
                                ))),
                        )
                        .child(
                            div().w_full().text_right().child(
                                Label::new(gpu.id.clone())
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                        ),
                )
        }))
}

fn render_storage_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap(px(14.))
        .when(telemetry.current.disks.is_empty(), |this| {
            this.child(empty_state("当前没有磁盘监控数据", cx))
        })
        .children(telemetry.current.disks.iter().map(|disk| {
            let disk_id = disk_key(disk);
            let history = telemetry
                .disk_history
                .get(&disk_id)
                .map(|items| items.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let (read_rate, write_rate) = latest_disk_rates(telemetry, &disk_id);
            let used_gb = disk_used_gb(disk);
            let disk_type = if disk.disk_type.is_empty() {
                "本地磁盘".to_string()
            } else {
                disk.disk_type.clone()
            };
            let manufacturer_label = if disk.manufacturer.is_empty() {
                "未知磁盘".to_string()
            } else {
                disk.manufacturer.clone()
            };

            card(format!("disk-panel-{disk_id}"))
                .p(px(16.))
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(
                    v_flex()
                        .gap(px(10.))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap(px(8.))
                                        .items_center()
                                        .child(monitor_icons::nav_icon(monitor_icons::STORAGE))
                                        .child(
                                            Label::new(disk.mount_on.clone())
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground),
                                        ),
                                )
                                .child(chip(&manufacturer_label, cx)),
                        )
                        .child(
                            metric_grid_row_equal()
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-usage"),
                                    "已用空间",
                                    format!("{used_gb:.1} / {} GB", disk.total_gb),
                                    Some(disk_usage_percent(disk)),
                                    Some(cx.theme().yellow),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-free"),
                                    "可用空间",
                                    format!("{} GB", disk.available_gb),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-type"),
                                    "磁盘类型",
                                    disk_type.clone(),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-read"),
                                    "读取速率",
                                    format!("{read_rate:.2} MB/s"),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-write"),
                                    "写入速率",
                                    format!("{write_rate:.2} MB/s"),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("disk-{disk_id}-fs"),
                                    "文件系统",
                                    if disk.filesystem.is_empty() {
                                        "—".to_string()
                                    } else {
                                        disk.filesystem.clone()
                                    },
                                    None,
                                    None,
                                    cx,
                                )),
                        )
                        .child(
                            h_flex()
                                .gap(px(14.))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("disk-{disk_id}-chart-read"),
                                    "读取速率",
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.read_mb,
                                    cx.theme().primary,
                                    "MB/s",
                                    "时间",
                                    None,
                                    true,
                                    true,
                                    cx,
                                )))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("disk-{disk_id}-chart-write"),
                                    "写入速率",
                                    history,
                                    |point| point.time.clone(),
                                    |point| point.write_mb,
                                    cx.theme().yellow,
                                    "MB/s",
                                    "时间",
                                    None,
                                    true,
                                    true,
                                    cx,
                                ))),
                        ),
                )
        }))
        .child(div().h(px(10.)))
}

fn render_network_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap(px(14.))
        .when(telemetry.current.networks.is_empty(), |this| {
            this.child(empty_state("当前没有网卡监控数据", cx))
        })
        .children(telemetry.current.networks.iter().map(|network| {
            let network_id = network_key(network);
            let history = telemetry
                .network_history
                .get(&network_id)
                .map(|items| items.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let (send_rate, recv_rate) = latest_network_rates(telemetry, &network_id);
            let ipv4 = network_ipv4(network);

            card(format!("network-panel-{network_id}"))
                .p(px(16.))
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(
                    v_flex()
                        .gap(px(10.))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap(px(8.))
                                        .items_center()
                                        .child(monitor_icons::nav_icon(monitor_icons::NETWORK))
                                        .child(
                                            Label::new(network.name.clone())
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground),
                                        ),
                                )
                                .child(chip(
                                    if ipv4.is_empty() { "未分配 IPv4" } else { "在线" },
                                    cx,
                                )),
                        )
                        .child(
                            metric_grid_row_equal()
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-ipv4"),
                                    "IPv4",
                                    if ipv4.is_empty() {
                                        "—".to_string()
                                    } else {
                                        ipv4.clone()
                                    },
                                    None,
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-mac"),
                                    "MAC",
                                    if network.mac.is_empty() {
                                        "—".to_string()
                                    } else {
                                        network.mac.clone()
                                    },
                                    None,
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-link"),
                                    "链路速率",
                                    format_network_link_speed(network),
                                    None,
                                    Some(cx.theme().primary),
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-send-rate"),
                                    "发送速率",
                                    format!("{send_rate:.2} MB/s"),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-recv-rate"),
                                    "接收速率",
                                    format!("{recv_rate:.2} MB/s"),
                                    None,
                                    None,
                                    cx,
                                ))
                                .child(render_overview_metric_card(
                                    &format!("net-{network_id}-total"),
                                    "累计流量",
                                    format!(
                                        "↑ {:.2} / ↓ {:.2} GB",
                                        bytes_to_gb(network.sent_data),
                                        bytes_to_gb(network.received_data)
                                    ),
                                    None,
                                    Some(cx.theme().primary),
                                    cx,
                                )),
                        )
                        .child(
                            h_flex()
                                .gap(px(14.))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("net-{network_id}-chart-send"),
                                    "发送速率",
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.send_mb,
                                    cx.theme().primary,
                                    "MB/s",
                                    "时间",
                                    None,
                                    true,
                                    true,
                                    cx,
                                )))
                                .child(div().flex_1().min_w_0().child(render_chart(
                                    &format!("net-{network_id}-chart-recv"),
                                    "接收速率",
                                    history,
                                    |point| point.time.clone(),
                                    |point| point.recv_mb,
                                    cx.theme().yellow,
                                    "MB/s",
                                    "时间",
                                    None,
                                    true,
                                    true,
                                    cx,
                                ))),
                        ),
                )
        }))
        .child(div().h(px(10.)))
}

fn render_table_header_cell<V>(label: &str, width: Pixels, cx: &Context<V>) -> impl IntoElement {
    div()
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .w(width)
        .child(
            Label::new(label.to_uppercase())
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground),
        )
}

fn render_table_header_flex_cell<V>(
    label: &str,
    min_w: Pixels,
    cx: &Context<V>,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(min_w)
        .h_full()
        .flex()
        .items_center()
        .child(
            Label::new(label.to_uppercase())
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground),
        )
}

fn render_scrollable_virtual_table_panel<V>(
    panel_id: &str,
    table_min_width: Pixels,
    h_scroll_handle: &VirtualListScrollHandle,
    v_scroll_handle: &VirtualListScrollHandle,
    header: impl IntoElement,
    body: impl IntoElement,
    footer_text: String,
    fill_width: bool,
    cx: &Context<V>,
) -> impl IntoElement {
    let h_offset_x = h_scroll_handle.offset().x;
    let table_scroll_size = size(table_min_width, px(1.));

    v_flex()
        .flex_1()
        .min_h_0()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(
            div()
                .id(format!("{panel_id}-table-scroll-area"))
                .flex_1()
                .min_h_0()
                .relative()
                .child(
                    v_flex()
                        .size_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .id(format!("{panel_id}-table-header-viewport"))
                                .flex_none()
                                .w_full()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .left(h_offset_x)
                                        .min_w(table_min_width)
                                        .when(fill_width, |this| this.w_full())
                                        .child(header),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("{panel_id}-table-body-viewport"))
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .overflow_hidden()
                                .relative()
                                .child(
                                    div()
                                        .w_full()
                                        .h_full()
                                        .pr(px(16.))
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .left(h_offset_x)
                                                .min_w(table_min_width)
                                                .when(fill_width, |this| this.w_full())
                                                .h_full()
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .when(fill_width, |this| this.w_full())
                                                        .child(body),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .bottom_0()
                                        .w(px(16.))
                                        .child(
                                            Scrollbar::vertical(v_scroll_handle)
                                                .scrollbar_show(ScrollbarShow::Always),
                                        ),
                                ),
                        ),
                )
                .child(ScrollableMask::new(
                    Axis::Horizontal,
                    h_scroll_handle.base_handle(),
                )),
        )
        .child(
            div()
                .flex_none()
                .w_full()
                .h(px(16.))
                .child(
                    Scrollbar::horizontal(h_scroll_handle)
                        .scrollbar_show(ScrollbarShow::Always)
                        .scroll_size(table_scroll_size),
                ),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    Label::new(footer_text)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
}

fn render_processes_tab<V: Render, F>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    h_scroll_handle: &VirtualListScrollHandle,
    process_search: &Entity<InputState>,
    process_sort: ProcessSort,
    on_cycle_sort: F,
    _window: &mut Window,
    cx: &mut Context<V>,
) -> impl IntoElement
where
    F: Fn(ProcessSortColumn, &mut Window, &mut App) + Clone + 'static,
{
    let query = process_search.read(cx).value().to_lowercase();
    let mut processes: Vec<SysProcessInfo> = telemetry
        .current
        .processes
        .iter()
        .filter(|process| {
            if query.is_empty() {
                return true;
            }
            process.name.to_lowercase().contains(&query)
                || process.command_line.to_lowercase().contains(&query)
                || process.exe.to_lowercase().contains(&query)
                || process.pid.to_string().contains(&query)
        })
        .cloned()
        .collect();
    let top_cpu = processes
        .iter()
        .max_by(|a, b| {
            a.cpu_usage
                .partial_cmp(&b.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let top_mem = processes
        .iter()
        .max_by_key(|process| process.memory)
        .cloned();
    sort_processes(&mut processes, process_sort);

    let processes: Arc<[SysProcessInfo]> = processes.into();

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row_equal()
                .child(render_process_header_metric_card(
                    "process-count",
                    "进程数",
                    processes.len().to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "process-top-cpu",
                    "最高 CPU",
                    top_cpu
                        .as_ref()
                        .map(|process| format!("{} {:.1}%", process.name, process.cpu_usage))
                        .unwrap_or_else(|| "-".to_string()),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "process-top-mem",
                    "最高内存",
                    top_mem
                        .as_ref()
                        .map(|process| format!("{} {} MB", process.name, process.memory_mb))
                        .unwrap_or_else(|| "-".to_string()),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(monitor_search_input(process_search)),
        )
        .child(render_scrollable_virtual_table_panel(
            "process",
            process_table_min_width(),
            h_scroll_handle,
            scroll_handle,
            render_process_table_header(process_sort, on_cycle_sort, cx),
            render_process_table(processes.clone(), scroll_handle, cx),
            format!("共 {} 个进程", processes.len()),
            false,
            cx,
        ))
        .pb(px(10.))
}

fn render_process_table_header<V: Render, F>(
    sort: ProcessSort,
    on_cycle_sort: F,
    cx: &mut Context<V>,
) -> impl IntoElement
where
    F: Fn(ProcessSortColumn, &mut Window, &mut App) + Clone + 'static,
{
    h_flex()
        .id("process-table-header")
        .min_w(process_table_min_width())
        .h(px(32.))
        .px(px(12.))
        .gap(PROCESS_COL_GAP)
        .items_center()
        .overflow_hidden()
        .bg(panel_2(cx))
        .border_b_1()
        .border_color(cx.theme().border)
        .child(render_header_cell(
            "PID",
            PROCESS_COL_PID,
            false,
            None,
            false,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "名称",
            PROCESS_COL_NAME,
            false,
            None,
            false,
            false,
            Some(ProcessSortColumn::Name),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "状态",
            PROCESS_COL_STATUS,
            false,
            None,
            false,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "CPU%",
            PROCESS_COL_CPU,
            false,
            None,
            true,
            false,
            Some(ProcessSortColumn::Cpu),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "内存 (MB)",
            PROCESS_COL_MEM,
            false,
            None,
            true,
            false,
            Some(ProcessSortColumn::Memory),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "虚拟内存 (MB)",
            PROCESS_COL_VMEM,
            false,
            None,
            true,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "读取 (MB/s)",
            PROCESS_COL_READ,
            false,
            None,
            true,
            false,
            Some(ProcessSortColumn::DiskRead),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "写入 (MB/s)",
            PROCESS_COL_WRITE,
            false,
            None,
            true,
            false,
            Some(ProcessSortColumn::DiskWrite),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "GPU",
            PROCESS_COL_GPU,
            false,
            None,
            false,
            true,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "GPU%",
            PROCESS_COL_GPU_USAGE,
            false,
            None,
            true,
            false,
            Some(ProcessSortColumn::Gpu),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "GPU 内存 (MB)",
            PROCESS_COL_GPU_MEM,
            false,
            None,
            true,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "命令行",
            PROCESS_COL_CMD,
            false,
            None,
            false,
            true,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
}

fn render_header_cell<V: Render, F>(
    label: &str,
    width: Pixels,
    flex: bool,
    min_w: Option<Pixels>,
    align_right: bool,
    leading_pad: bool,
    column: Option<ProcessSortColumn>,
    sort: ProcessSort,
    on_cycle_sort: F,
    cx: &mut Context<V>,
) -> Stateful<gpui::Div>
where
    F: Fn(ProcessSortColumn, &mut Window, &mut App) + Clone + 'static,
{
    let active = column.is_some_and(|column| column == sort.column);
    let arrow = if active {
        match sort.direction {
            SortDirection::Asc => " ↑",
            SortDirection::Desc => " ↓",
        }
    } else {
        ""
    };
    let upper = label.to_uppercase();
    let text = format!("{upper}{arrow}");
    let mut cell = div()
        .id(format!("process-header-{label}"))
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .text_xs()
        .font_semibold()
        .child(
            Label::new(text)
                .text_xs()
                .font_semibold()
                .text_color(if active {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                }),
        );
    if width > px(0.) {
        cell = cell.w(width);
    }
    if flex {
        cell = cell.flex_1();
    }
    if let Some(min_w) = min_w {
        cell = cell.min_w(min_w);
    }
    if align_right {
        cell = cell
            .justify_end()
            .text_right()
            .pl(PROCESS_COL_INNER_PAD);
    }
    if leading_pad {
        cell = cell.pl(PROCESS_COL_INNER_PAD);
    }
    if let Some(column) = column {
        cell = cell.cursor_pointer().on_click(move |_event, window, cx| {
            on_cycle_sort(column, window, cx);
        });
    }
    cell
}

fn render_process_table<V: Render>(
    processes: Arc<[SysProcessInfo]>,
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let item_count = processes.len().max(1);
    let item_sizes =
        Rc::new(vec![size(process_table_min_width(), px(32.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "process-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    processes
                        .get(index)
                        .map(|process| render_process_row(process, index, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_process_row<V>(
    process: &SysProcessInfo,
    index: usize,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let pid = process.pid;
    let status_color = if process.status == "运行中" {
        cx.theme().primary
    } else {
        cx.theme().foreground
    };
    let stripe_bg = striped_row_bg(index, cx);
    h_flex()
        .id(format!("process-row-{}", process.pid))
        .when_some(stripe_bg, |this, bg| this.bg(bg))
        .hover(|style| style.bg(cx.theme().primary.opacity(0.07)))
        .context_menu(move |menu, window, cx| {
            menu.menu("结束任务", Box::new(TerminateProcess { pid }))
                .menu("结束进程树", Box::new(TerminateProcessTree { pid }))
                .menu("打开文件位置", Box::new(RevealProcessExe { pid }))
                .menu("属性", Box::new(ShowProcessDetails { pid }))
                .menu("拷贝信息", Box::new(CopyProcessInfo { pid }))
                .separator()
                .menu("暂停", Box::new(SuspendProcess { pid }))
                .menu("恢复", Box::new(ResumeProcess { pid }))
                .separator()
                .submenu("优先级", window, cx, move |menu, _window, _cx| {
                    menu.menu(
                        "实时",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "realtime".to_string(),
                        }),
                    )
                    .menu(
                        "高",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "high".to_string(),
                        }),
                    )
                    .menu(
                        "高于标准",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "above_normal".to_string(),
                        }),
                    )
                    .menu(
                        "标准",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "normal".to_string(),
                        }),
                    )
                    .menu(
                        "低于标准",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "below_normal".to_string(),
                        }),
                    )
                    .menu(
                        "低",
                        Box::new(SetProcessPriority {
                            pid,
                            priority: "idle".to_string(),
                        }),
                    )
                })
                .submenu("I/O 优先级", window, cx, move |menu, _window, _cx| {
                    menu.menu(
                        "高",
                        Box::new(SetProcessIoPriority {
                            pid,
                            priority: "high".to_string(),
                        }),
                    )
                    .menu(
                        "标准",
                        Box::new(SetProcessIoPriority {
                            pid,
                            priority: "normal".to_string(),
                        }),
                    )
                    .menu(
                        "低",
                        Box::new(SetProcessIoPriority {
                            pid,
                            priority: "low".to_string(),
                        }),
                    )
                    .menu(
                        "很低",
                        Box::new(SetProcessIoPriority {
                            pid,
                            priority: "very_low".to_string(),
                        }),
                    )
                })
                .submenu("CPU 亲和性", window, cx, move |menu, _window, _cx| {
                    menu.menu(
                        "所有核心",
                        Box::new(SetProcessAffinity {
                            pid,
                            affinity_mask: u64::MAX,
                        }),
                    )
                    .menu(
                        "仅 CPU 0",
                        Box::new(SetProcessAffinity {
                            pid,
                            affinity_mask: 1,
                        }),
                    )
                    .menu(
                        "CPU 0-1",
                        Box::new(SetProcessAffinity {
                            pid,
                            affinity_mask: 3,
                        }),
                    )
                    .menu(
                        "CPU 0-3",
                        Box::new(SetProcessAffinity {
                            pid,
                            affinity_mask: 0xF,
                        }),
                    )
                })
        })
        .min_w(process_table_min_width())
        .h(px(32.))
        .px(px(12.))
        .gap(PROCESS_COL_GAP)
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(process_table_text_cell(
            process.pid.to_string(),
            PROCESS_COL_PID,
            false,
        ))
        .child(process_table_text_cell(
            process.name.clone(),
            PROCESS_COL_NAME,
            false,
        ))
        .child(
            div()
                .w(PROCESS_COL_STATUS)
                .flex_none()
                .overflow_hidden()
                .child(
                    Label::new(process.status.clone())
                        .text_sm()
                        .truncate()
                        .text_color(status_color),
                ),
        )
        .child(process_table_text_cell(
            format!("{:.1}", process.cpu_usage),
            PROCESS_COL_CPU,
            true,
        ))
        .child(process_table_text_cell(
            format!("{}", process.memory_mb),
            PROCESS_COL_MEM,
            true,
        ))
        .child(process_table_text_cell(
            format!("{}", process.virtual_memory_mb),
            PROCESS_COL_VMEM,
            true,
        ))
        .child(process_table_text_cell(
            format!("{:.2}", process.disk_read_rate),
            PROCESS_COL_READ,
            true,
        ))
        .child(process_table_text_cell(
            format!("{:.2}", process.disk_write_rate),
            PROCESS_COL_WRITE,
            true,
        ))
        .child(process_table_text_cell_after_numeric(
            if process.gpu_name.is_empty() {
                "—".to_string()
            } else {
                process.gpu_name.clone()
            },
            PROCESS_COL_GPU,
        ))
        .child(process_table_text_cell(
            if process.gpu_usage > 0.0 {
                format!("{:.1}", process.gpu_usage)
            } else {
                "—".to_string()
            },
            PROCESS_COL_GPU_USAGE,
            true,
        ))
        .child(process_table_text_cell(
            if process.gpu_dedicated_bytes > 0 {
                format!("{}", process.gpu_dedicated_bytes / 1024 / 1024)
            } else {
                "—".to_string()
            },
            PROCESS_COL_GPU_MEM,
            true,
        ))
        .child(process_table_cmd_cell(
            if process.command_line.is_empty() {
                process.exe.clone()
            } else {
                process.command_line.clone()
            },
            PROCESS_COL_CMD,
        ))
}

fn process_table_text_cell(value: String, width: Pixels, align_right: bool) -> gpui::Div {
    virtual_table_text_cell(value, width, align_right)
}

fn virtual_table_text_cell(value: String, width: Pixels, align_right: bool) -> gpui::Div {
    let mut cell = div()
        .w(width)
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .child(Label::new(value).text_sm().truncate());
    if align_right {
        cell = cell
            .justify_end()
            .text_right()
            .pl(PROCESS_COL_INNER_PAD);
    } else {
        cell = cell.pr(PROCESS_COL_INNER_PAD);
    }
    cell
}

fn process_table_text_cell_after_numeric(value: String, width: Pixels) -> gpui::Div {
    div()
        .w(width)
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .pl(PROCESS_COL_INNER_PAD)
        .child(Label::new(value).text_sm().truncate())
}

fn process_table_cmd_cell(value: String, width: Pixels) -> gpui::Div {
    div()
        .w(width)
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .pl(PROCESS_COL_INNER_PAD)
        .child(Label::new(value).text_sm().truncate())
}

fn inline_stat_row<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    h_flex()
        .child(
            Label::new(format!("{label}: "))
                .text_sm()
                .text_color(cx.theme().foreground),
        )
        .child(
            Label::new(value.to_string())
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().foreground),
        )
}

fn info_item<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .p(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(panel_2(cx))
        .child(
            Label::new(label.to_uppercase())
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div().mt(px(6.)).child(
                Label::new(value.to_string())
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .line_height(px(20.)),
            ),
        )
}

fn empty_state<V>(message: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .id("empty-state")
        .w_full()
        .h(px(260.))
        .mt(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .border_1()
        .border_color(border_strong(cx))
        .bg(cx.theme().secondary)
        .child(
            Label::new(message.to_string())
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
}

fn render_services_tab<V: Render>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    h_scroll_handle: &VirtualListScrollHandle,
    service_search: &Entity<InputState>,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let query = service_search.read(cx).value().to_lowercase();
    let services: Vec<SysServiceInfo> = telemetry
        .current
        .services
        .iter()
        .filter(|service| {
            if query.is_empty() {
                return true;
            }
            service.name.to_lowercase().contains(&query)
                || service.display_name.to_lowercase().contains(&query)
                || service.status.to_lowercase().contains(&query)
                || service.start_type.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();

    let running = services.iter().filter(|s| s.status == "运行中").count();
    let stopped = services.iter().filter(|s| s.status == "已停止").count();
    let service_count = services.len();
    let services: Arc<[SysServiceInfo]> = services.into();

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row_equal()
                .child(render_process_header_metric_card(
                    "service-count",
                    "服务数",
                    service_count.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "service-running",
                    "运行中",
                    running.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "service-stopped",
                    "已停止",
                    stopped.to_string(),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(monitor_search_input(service_search)),
        )
        .child(render_scrollable_virtual_table_panel(
            "service",
            service_table_min_width(),
            h_scroll_handle,
            scroll_handle,
            render_service_table_header(cx),
            render_service_table(services.clone(), scroll_handle, cx),
            format!("共 {} 个服务", service_count),
            true,
            cx,
        ))
        .pb(px(10.))
}

fn render_service_table_header<V: Render>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .id("service-table-header")
        .min_w(service_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(panel_2(cx))
        .border_b_1()
        .border_color(cx.theme().border)
        .child(render_table_header_cell("名称", SERVICE_COL_NAME, cx))
        .child(render_table_header_flex_cell("显示名称", SERVICE_COL_DISPLAY, cx))
        .child(render_table_header_cell("状态", SERVICE_COL_STATUS, cx))
        .child(render_table_header_cell("启动类型", SERVICE_COL_START, cx))
}

fn render_service_table<V: Render>(
    services: Arc<[SysServiceInfo]>,
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let item_count = services.len().max(1);
    let item_sizes = Rc::new(vec![size(px(0.), px(32.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "service-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    services
                        .get(index)
                        .map(|service| render_service_row(service, index, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_service_row<V: Render>(
    service: &SysServiceInfo,
    index: usize,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let name = service.name.clone();
    let status_color = service_status_color(&service.status, cx);
    let stripe_bg = striped_row_bg(index, cx);
    h_flex()
        .id(format!("service-row-{}", service.name))
        .when_some(stripe_bg, |this, bg| this.bg(bg))
        .hover(|style| style.bg(cx.theme().primary.opacity(0.07)))
        .context_menu(move |menu, _window, _cx| {
            menu.menu("启动", Box::new(StartServiceAction { name: name.clone() }))
                .menu("停止", Box::new(StopServiceAction { name: name.clone() }))
                .menu(
                    "重新启动",
                    Box::new(RestartServiceAction { name: name.clone() }),
                )
        })
        .min_w(service_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(virtual_table_text_cell(
            service.name.clone(),
            SERVICE_COL_NAME,
            false,
        ))
        .child(
            div()
                .flex_1()
                .min_w(SERVICE_COL_DISPLAY)
                .overflow_hidden()
                .flex()
                .items_center()
                .child(
                    Label::new(service.display_name.clone())
                        .text_sm()
                        .truncate(),
                ),
        )
        .child(
            div()
                .w(SERVICE_COL_STATUS)
                .flex_none()
                .overflow_hidden()
                .flex()
                .items_center()
                .child(
                    Label::new(service.status.clone())
                        .text_sm()
                        .truncate()
                        .text_color(status_color),
                ),
        )
        .child(virtual_table_text_cell(
            service.start_type.clone(),
            SERVICE_COL_START,
            false,
        ))
}

fn render_users_tab<V: Render>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    h_scroll_handle: &VirtualListScrollHandle,
    user_search: &Entity<InputState>,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let query = user_search.read(cx).value().to_lowercase();
    let users: Vec<SysUserInfo> = telemetry
        .current
        .users
        .iter()
        .filter(|user| {
            if query.is_empty() {
                return true;
            }
            user.name.to_lowercase().contains(&query)
                || user.uid.to_lowercase().contains(&query)
                || user.gid.to_lowercase().contains(&query)
                || user.groups.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();

    let admin_count = users
        .iter()
        .filter(|user| user.groups.to_lowercase().contains("administrators"))
        .count();
    let system_count = users
        .iter()
        .filter(|user| {
            user.groups.to_lowercase().contains("system") || user.name.to_lowercase() == "system"
        })
        .count();
    let user_count = users.len();
    let users: Arc<[SysUserInfo]> = users.into();

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row_equal()
                .child(render_process_header_metric_card(
                    "user-count",
                    "用户数",
                    user_count.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "user-admin",
                    "管理员账户",
                    admin_count.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "user-system",
                    "系统账户",
                    system_count.to_string(),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(monitor_search_input(user_search)),
        )
        .child(render_scrollable_virtual_table_panel(
            "user",
            user_table_min_width(),
            h_scroll_handle,
            scroll_handle,
            render_user_table_header(cx),
            render_user_table(users.clone(), scroll_handle, cx),
            format!("共 {} 个用户", user_count),
            true,
            cx,
        ))
        .pb(px(10.))
}

fn render_user_table_header<V: Render>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .id("user-table-header")
        .min_w(user_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(panel_2(cx))
        .border_b_1()
        .border_color(cx.theme().border)
        .child(render_table_header_cell("名称", USER_COL_NAME, cx))
        .child(render_table_header_cell("UID", USER_COL_UID, cx))
        .child(render_table_header_cell("GID", USER_COL_GID, cx))
        .child(render_table_header_cell("所属组", USER_COL_GROUPS, cx))
}

fn render_user_table<V: Render>(
    users: Arc<[SysUserInfo]>,
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let item_count = users.len().max(1);
    let item_sizes = Rc::new(vec![size(px(0.), px(32.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "user-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    users
                        .get(index)
                        .map(|user| render_user_row(user, index, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_user_row<V: Render>(
    user: &SysUserInfo,
    index: usize,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let stripe_bg = striped_row_bg(index, cx);
    h_flex()
        .id(format!("user-row-{}", user.name))
        .when_some(stripe_bg, |this, bg| this.bg(bg))
        .hover(|style| style.bg(cx.theme().primary.opacity(0.07)))
        .min_w(user_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(virtual_table_text_cell(user.name.clone(), USER_COL_NAME, false))
        .child(virtual_table_text_cell(user.uid.clone(), USER_COL_UID, false))
        .child(virtual_table_text_cell(user.gid.clone(), USER_COL_GID, false))
        .child(virtual_table_text_cell(user.groups.clone(), USER_COL_GROUPS, false))
}

fn service_status_color<V>(status: &str, cx: &Context<V>) -> Hsla {
    match status {
        "运行中" => cx.theme().primary,
        "已停止" => cx.theme().red,
        "已暂停" | "正在启动" | "正在停止" | "正在暂停" | "正在恢复" => {
            cx.theme().yellow
        }
        _ => cx.theme().muted_foreground,
    }
}

fn render_startup_tab<V: Render>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    h_scroll_handle: &VirtualListScrollHandle,
    startup_search: &Entity<InputState>,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let query = startup_search.read(cx).value().to_lowercase();
    let items: Vec<SysStartupInfo> = telemetry
        .current
        .startup_items
        .iter()
        .filter(|item| {
            if query.is_empty() {
                return true;
            }
            item.name.to_lowercase().contains(&query)
                || item.command.to_lowercase().contains(&query)
                || item.location.to_lowercase().contains(&query)
        })
        .cloned()
        .collect();

    let registry_count = items
        .iter()
        .filter(|item| item.location.starts_with("HK"))
        .count();
    let folder_count = items.len().saturating_sub(registry_count);
    let item_count = items.len();
    let items: Arc<[SysStartupInfo]> = items.into();

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row_equal()
                .child(render_process_header_metric_card(
                    "startup-count",
                    "启动项数",
                    item_count.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "startup-registry",
                    "注册表项",
                    registry_count.to_string(),
                    cx,
                ))
                .child(render_process_header_metric_card(
                    "startup-folder",
                    "启动文件夹项",
                    folder_count.to_string(),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(monitor_search_input(startup_search)),
        )
        .child(render_scrollable_virtual_table_panel(
            "startup",
            startup_table_min_width(),
            h_scroll_handle,
            scroll_handle,
            render_startup_table_header(cx),
            render_startup_table(items.clone(), scroll_handle, cx),
            format!("共 {} 个启动项", item_count),
            true,
            cx,
        ))
        .pb(px(10.))
}

fn render_startup_table_header<V: Render>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .id("startup-table-header")
        .min_w(startup_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(panel_2(cx))
        .border_b_1()
        .border_color(cx.theme().border)
        .child(render_table_header_cell("名称", STARTUP_COL_NAME, cx))
        .child(render_table_header_cell("命令", STARTUP_COL_COMMAND, cx))
        .child(render_table_header_flex_cell("位置", STARTUP_COL_LOCATION, cx))
}

fn render_startup_table<V: Render>(
    items: Arc<[SysStartupInfo]>,
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let item_count = items.len().max(1);
    let item_sizes = Rc::new(vec![size(px(0.), px(32.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "startup-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    items
                        .get(index)
                        .map(|item| render_startup_row(item, index, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_startup_row<V: Render>(
    item: &SysStartupInfo,
    index: usize,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let command = item.command.clone();
    let stripe_bg = striped_row_bg(index, cx);
    h_flex()
        .id(format!("startup-row-{}", item.name))
        .when_some(stripe_bg, |this, bg| this.bg(bg))
        .hover(|style| style.bg(cx.theme().primary.opacity(0.07)))
        .context_menu(move |menu, _window, _cx| {
            menu.menu(
                "打开文件位置",
                Box::new(RevealStartupItem {
                    command: command.clone(),
                }),
            )
        })
        .min_w(startup_table_min_width())
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(virtual_table_text_cell(item.name.clone(), STARTUP_COL_NAME, false))
        .child(virtual_table_text_cell(item.command.clone(), STARTUP_COL_COMMAND, false))
        .child(
            div()
                .flex_1()
                .min_w(STARTUP_COL_LOCATION)
                .overflow_hidden()
                .flex()
                .items_center()
                .child(Label::new(item.location.clone()).text_sm().truncate()),
        )
}
