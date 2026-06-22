use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, relative, rgb, size,
    AnyElement, App, Context, ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window,
};
use gpui_component::{
    chart::AreaChart,
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    table::{Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow},
    v_flex, v_virtual_list, ActiveTheme, Sizable, StyledExt, VirtualListScrollHandle,
};

use crate::monitor_actions::{
    RestartServiceAction, ResumeProcess, RevealProcessExe, RevealStartupItem, SetProcessAffinity,
    SetProcessIoPriority, SetProcessPriority, ShowProcessDetails, StartServiceAction,
    StopServiceAction, SuspendProcess, TerminateProcess, TerminateProcessTree,
};
use crate::monitor_icons;
use crate::monitor_model::{
    bytes_to_gb, chart_ticks, disk_usage_percent, format_optional_frequency, format_tick, gpu_key,
    gpu_memory_percent, network_ipv4, sort_processes, MachineTelemetry, MonitorTab, ProcessSort,
    ProcessSortColumn, SortDirection,
};
use crate::sys_info::{SysProcessInfo, SysServiceInfo, SysStartupInfo, SysUserInfo};
use app_ui::ContextMenuExt;

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
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(cx.theme().muted.opacity(0.1))
                .border_color(cx.theme().border)
        })
        .child(monitor_icons::icon(icon_path).text_color(cx.theme().foreground))
}

pub fn render_dashboard<V: Render, F>(
    telemetry: &MachineTelemetry,
    active_tab: MonitorTab,
    process_scroll_handle: &VirtualListScrollHandle,
    process_search: &Entity<InputState>,
    process_sort: ProcessSort,
    service_scroll_handle: &VirtualListScrollHandle,
    service_search: &Entity<InputState>,
    startup_scroll_handle: &VirtualListScrollHandle,
    startup_search: &Entity<InputState>,
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
            process_search,
            process_sort,
            on_cycle_sort,
            window,
            cx,
        )
        .into_any_element(),
        MonitorTab::Services => {
            render_services_tab(telemetry, service_scroll_handle, service_search, cx)
                .into_any_element()
        }
        MonitorTab::Startup => {
            render_startup_tab(telemetry, startup_scroll_handle, startup_search, cx)
                .into_any_element()
        }
        MonitorTab::Users => render_users_tab(telemetry, user_search, cx).into_any_element(),
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

    v_flex()
        .id("monitor-nav")
        .gap(px(7.))
        .children(tabs.iter().map(|(tab, label, path)| {
            let tab = *tab;
            let on_click = on_click.clone();
            let is_active = active_tab == tab;
            let text_color = if is_active {
                cx.theme().primary
            } else {
                cx.theme().foreground
            };
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
                .border_color(if is_active {
                    cx.theme().border
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
                    this.hover(|style| style.bg(hover_bg).border_color(cx.theme().border))
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
                                .child(monitor_icons::icon(path).text_color(text_color)),
                        )
                        .child(Label::new(*label).text_sm().text_color(text_color)),
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
    let value_is_long = value.len() > 14;
    let percent = percent.unwrap_or(0.0).clamp(0.0, 100.0);
    let progress_color = progress_color.unwrap_or(cx.theme().primary);

    card(id)
        .flex_1()
        .min_w(px(160.))
        .h(px(96.))
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
                .gap_1()
                .child(
                    Label::new(value)
                        .when(value_is_long, |this| this.text_sm())
                        .when(!value_is_long, |this| this.text_2xl())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(value_is_long.then_some(20.).unwrap_or(28.)))
                        .text_color(cx.theme().foreground),
                )
                .child(
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
                                .w(relative(percent / 100.0)),
                        ),
                ),
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
    let chart_min_h = if compact { px(160.) } else { px(300.) };
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
            linear_color_stop(cx.theme().background.opacity(0.05), 0.),
        ))
        .tick_margin(x_tick_margin)
        .x_axis(show_x_axis);
    if let Some(y_max) = y_max {
        chart = chart
            .y(move |_| y_max)
            .stroke(color.opacity(0.0))
            .fill(cx.theme().background.opacity(0.0));
    }

    v_flex()
        .id(SharedString::from(id.to_string()))
        .h(chart_min_h)
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
                        .child(
                            div()
                                .w(px(8.))
                                .h(px(8.))
                                .rounded_full()
                                .bg(color)
                                .shadow_md(),
                        )
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
                .child(div().flex_1().h_full().relative().child(chart).when(
                    show_x_axis && !x_labels.is_empty(),
                    |this| {
                        this.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .h(px(18.))
                                .child(h_flex().w_full().h_full().children(
                                    x_labels.iter().enumerate().map(|(i, text)| {
                                        let last = x_label_count - 1;
                                        div()
                                            .flex_1()
                                            .h_full()
                                            .flex()
                                            .items_end()
                                            .when(i == 0, |this| this.justify_start())
                                            .when(i == last, |this| this.justify_end())
                                            .when(i > 0 && i < last, |this| this.justify_center())
                                            .child(
                                                Label::new(text.clone())
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                    }),
                                )),
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

fn metric_grid_row() -> gpui::Div {
    h_flex().gap(px(14.)).flex_wrap()
}

fn render_overview_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let send_rate = telemetry.latest_send_rate();
    let recv_rate = telemetry.latest_recv_rate();
    let send_percent = ((send_rate / 10.0) * 100.0).min(100.0) as f32;
    let recv_percent = ((recv_rate / 10.0) * 100.0).min(100.0) as f32;

    v_flex()
        .gap(px(14.))
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "overview-cpu",
                    "CPU 使用率",
                    format!("{:.1}%", telemetry.latest_cpu_percent()),
                    Some(telemetry.latest_cpu_percent()),
                    Some(cx.theme().red),
                    cx,
                ))
                .child(render_metric_card(
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
                .child(render_metric_card(
                    "overview-disk",
                    "磁盘最高占用",
                    format!("{:.1}%", telemetry.highest_disk_percent()),
                    Some(telemetry.highest_disk_percent()),
                    Some(cx.theme().yellow),
                    cx,
                ))
                .child(render_metric_card(
                    "overview-gpu",
                    "首块 GPU",
                    telemetry
                        .current
                        .gpus
                        .first()
                        .map(|gpu| gpu.brand.clone())
                        .unwrap_or_else(|| "无 GPU 数据".to_string()),
                    telemetry
                        .current
                        .gpus
                        .first()
                        .map(|gpu| gpu.gpu_utilization as f32)
                        .or(Some(8.0)),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "overview-send",
                    "主网卡发送速率",
                    format!("{:.2} MB/s", send_rate),
                    Some(send_percent),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "overview-recv",
                    "主网卡接收速率",
                    format!("{:.2} MB/s", recv_rate),
                    Some(recv_percent),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap(px(14.))
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "overview-chart-cpu",
                    "CPU 趋势",
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
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
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
        .child(
            h_flex()
                .gap(px(14.))
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
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
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
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
        )
}

fn render_cpu_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let cpu_usage = telemetry.current.cpu.usage;
    let core_count = telemetry.current.cpu.cpus.len();

    v_flex()
        .gap(px(14.))
        .child(
            h_flex()
                .gap(px(14.))
                .flex_wrap()
                .child(
                    card("cpu-usage")
                        .w(px(280.))
                        .flex_none()
                        .p(px(16.))
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .child(
                            Label::new("当前使用率".to_uppercase())
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div().mt(px(10.)).child(
                                Label::new(format!("{:.1}%", cpu_usage))
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground),
                            ),
                        )
                        .child(
                            div()
                                .mt(px(13.))
                                .w_full()
                                .h(px(6.))
                                .rounded_full()
                                .overflow_hidden()
                                .bg(cx.theme().primary.opacity(0.10))
                                .child(
                                    div()
                                        .h_full()
                                        .rounded_full()
                                        .bg(cx.theme().red)
                                        .w(relative((cpu_usage as f32 / 100.0).clamp(0.0, 1.0))),
                                ),
                        )
                        .child(div().mt(px(16.)).child(helper_row(
                            "逻辑核心",
                            &core_count.to_string(),
                            cx,
                        )))
                        .child(div().mt(px(10.)).child(helper_row(
                            "当前频率",
                            &format_optional_frequency(telemetry.current.cpu.current_frequency),
                            cx,
                        )))
                        .child(div().mt(px(10.)).child(helper_row(
                            "最大频率",
                            &format!("{:.2} GHz", telemetry.current.cpu.max_frequency),
                            cx,
                        ))),
                )
                .child(
                    card("cpu-info")
                        .flex_1()
                        .min_w(px(320.))
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
                                        .child(info_item("型号", &telemetry.current.cpu.brand, cx))
                                        .child(info_item(
                                            "Vendor",
                                            &telemetry.current.cpu.vendor,
                                            cx,
                                        ))
                                        .child(info_item(
                                            "架构",
                                            &format!("x64 / {} Threads", core_count),
                                            cx,
                                        )),
                                )
                                .child(
                                    h_flex()
                                        .gap(px(16.))
                                        .child(info_item(
                                            "基准频率",
                                            &format!(
                                                "{:.2} GHz",
                                                telemetry.current.cpu.base_frequency
                                            ),
                                            cx,
                                        ))
                                        .child(info_item(
                                            "最大频率",
                                            &format!(
                                                "{:.2} GHz",
                                                telemetry.current.cpu.max_frequency
                                            ),
                                            cx,
                                        ))
                                        .child(info_item(
                                            "总使用率",
                                            &format!("{:.1}%", cpu_usage),
                                            cx,
                                        )),
                                ),
                        ),
                ),
        )
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
        ))
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
                                &format!("Core {index}"),
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
}

fn metric_item<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .p(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            Label::new(label.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div().mt(px(6.)).child(
                Label::new(value.to_string())
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
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
                        .child(metric_item(
                            "总内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.total)),
                            cx,
                        ))
                        .child(metric_item(
                            "已用内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.used)),
                            cx,
                        ))
                        .child(metric_item(
                            "可用内存",
                            &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.available)),
                            cx,
                        ))
                        .child(metric_item("使用率", &format!("{:.1}%", mem_percent), cx)),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .w_full()
                        .h(px(6.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(cx.theme().primary.opacity(0.10))
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(cx.theme().primary)
                                .w(relative((mem_percent / 100.0).clamp(0.0, 1.0))),
                        ),
                ),
        )
        .child(
            h_flex()
                .gap(px(14.))
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
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
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
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
        .child(empty_state(
            "这里可以扩展：分页池、非分页池、缓存、提交大小、交换区等高级指标",
            cx,
        ))
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
                .gap(px(14.))
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
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
                    metric_grid_row()
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-usage"),
                            "利用率",
                            format!("{}%", gpu.gpu_utilization),
                            Some(gpu.gpu_utilization as f32),
                            Some(cx.theme().primary),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-temp"),
                            "温度",
                            format!("{} °C", gpu.temperature),
                            Some((gpu.temperature as f32).clamp(0.0, 100.0)),
                            Some(cx.theme().red),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-fan"),
                            "风扇转速",
                            format!("{} RPM", gpu.fan_speed),
                            Some(((gpu.fan_speed as f32 / 5000.0) * 100.0).clamp(0.0, 100.0)),
                            Some(cx.theme().primary),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-vram"),
                            "显存",
                            format!("{:.2} / {:.2} GB", gpu.mem_used_gb, gpu.mem_total_gb),
                            Some(gpu_memory_percent(gpu)),
                            Some(cx.theme().primary),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-encoder"),
                            "编码器",
                            format!("{}%", gpu.encoder_utilization),
                            Some(gpu.encoder_utilization as f32),
                            Some(cx.theme().primary),
                            cx,
                        ))
                        .child(render_metric_card(
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
                        .gap(px(12.))
                        .flex_wrap()
                        .child(div().flex_1().min_w(px(280.)).child(render_chart(
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
                        .child(div().flex_1().min_w(px(280.)).child(render_chart(
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
                        )))
                        .child(div().flex_1().min_w(px(280.)).child(render_chart(
                            &format!("gpu-{gpu_id}-chart-mem"),
                            "显存占用",
                            history,
                            |point| point.time.clone(),
                            |point| point.memory_used_gb,
                            cx.theme().primary,
                            "GB",
                            "时间",
                            Some(gpu.mem_total_gb as f64),
                            true,
                            true,
                            cx,
                        ))),
                )
                .child(
                    div().mt(px(10.)).w_full().text_right().child(
                        Label::new(gpu.id.clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
                )
        }))
}

fn render_storage_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let read_rate = telemetry.latest_disk_read_rate();
    let write_rate = telemetry.latest_disk_write_rate();
    v_flex()
        .gap(px(14.))
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "storage-disk-count",
                    "磁盘数量",
                    telemetry.current.disks.len().to_string(),
                    Some(30.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "storage-max-used",
                    "最高占用",
                    format!("{:.1}%", telemetry.highest_disk_percent()),
                    Some(telemetry.highest_disk_percent()),
                    Some(cx.theme().yellow),
                    cx,
                ))
                .child(render_metric_card(
                    "storage-read-rate",
                    "磁盘读取速率",
                    format!("{:.2} MB/s", read_rate),
                    Some(((read_rate / 10.0) * 100.0).min(100.0) as f32),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "storage-write-rate",
                    "磁盘写入速率",
                    format!("{:.2} MB/s", write_rate),
                    Some(((write_rate / 10.0) * 100.0).min(100.0) as f32),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(render_disk_table(telemetry, cx))
}

fn render_network_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let primary = telemetry.current.networks.first();
    let send_rate = telemetry.latest_send_rate();
    let recv_rate = telemetry.latest_recv_rate();
    v_flex()
        .gap(px(14.))
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "network-primary-name",
                    "当前主网卡",
                    primary
                        .map(|item| item.name.clone())
                        .unwrap_or_else(|| "未识别".to_string()),
                    Some(20.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "network-send-rate",
                    "发送速率",
                    format!("{:.2} MB/s", send_rate),
                    Some(((send_rate / 10.0) * 100.0).min(100.0) as f32),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "network-recv-rate",
                    "接收速率",
                    format!("{:.2} MB/s", recv_rate),
                    Some(((recv_rate / 10.0) * 100.0).min(100.0) as f32),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap(px(14.))
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "network-chart-send",
                    "发送速率",
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
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "network-chart-recv",
                    "接收速率",
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
        )
        .child(render_network_table(telemetry, cx))
}

fn render_disk_table<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("挂载点"))
                    .child(TableHead::new().child("类型"))
                    .child(TableHead::new().child("文件系统"))
                    .child(TableHead::new().text_right().child("总量(GB)"))
                    .child(TableHead::new().text_right().child("可用(GB)"))
                    .child(TableHead::new().text_right().child("已用(%)"))
                    .child(TableHead::new().text_right().child("读取(MB/s)"))
                    .child(TableHead::new().text_right().child("写入(MB/s)")),
            ),
        )
        .child(
            TableBody::new().children(telemetry.current.disks.iter().map(|disk| {
                TableRow::new()
                    .child(TableCell::new().child(disk.mount_on.clone()))
                    .child(TableCell::new().child(disk.disk_type.clone()))
                    .child(TableCell::new().child(disk.filesystem.clone()))
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{}", disk.total_gb)),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{}", disk.available_gb)),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.1}%", disk_usage_percent(disk))),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.2}", disk.read_rate)),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.2}", disk.write_rate)),
                    )
            })),
        )
        .child(TableCaption::new().child(format!("共 {} 块磁盘", telemetry.current.disks.len())))
        .bg(cx.theme().table)
}

fn render_network_table<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("名称"))
                    .child(TableHead::new().child("MAC"))
                    .child(TableHead::new().child("IPv4"))
                    .child(TableHead::new().text_right().child("累计发送(GB)"))
                    .child(TableHead::new().text_right().child("累计接收(GB)"))
                    .child(TableHead::new().text_right().child("链路速率(Mbps)")),
            ),
        )
        .child(
            TableBody::new().children(telemetry.current.networks.iter().map(|network| {
                TableRow::new()
                    .child(TableCell::new().child(network.name.clone()))
                    .child(TableCell::new().child(network.mac.clone()))
                    .child(TableCell::new().child(network_ipv4(network)))
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.2}", bytes_to_gb(network.sent_data))),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.2}", bytes_to_gb(network.received_data))),
                    )
                    .child(TableCell::new().text_right().child(format!(
                        "{}/{}",
                        network.max_transmit_speed, network.max_receive_speed
                    )))
            })),
        )
        .child(TableCaption::new().child("展示所有非虚拟网卡数据"))
        .bg(cx.theme().table)
}

fn render_processes_tab<V: Render, F>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
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
    sort_processes(&mut processes, process_sort);

    let top_cpu = processes.first().cloned();
    let top_mem = processes
        .iter()
        .max_by_key(|process| process.memory)
        .cloned();
    let processes: Arc<[SysProcessInfo]> = processes.into();
    let mem_total_mb = bytes_to_gb(telemetry.current.mem.total) * 1024.0;

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "process-count",
                    "进程数",
                    processes.len().to_string(),
                    Some(80.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "process-top-cpu",
                    "最高 CPU",
                    top_cpu
                        .as_ref()
                        .map(|process| format!("{} {:.1}%", process.name, process.cpu_usage))
                        .unwrap_or_else(|| "-".to_string()),
                    top_cpu
                        .as_ref()
                        .map(|process| process.cpu_usage.clamp(0.0, 100.0)),
                    Some(cx.theme().red),
                    cx,
                ))
                .child(render_metric_card(
                    "process-top-mem",
                    "最高内存",
                    top_mem
                        .as_ref()
                        .map(|process| format!("{} {} MB", process.name, process.memory_mb))
                        .unwrap_or_else(|| "-".to_string()),
                    top_mem.as_ref().map(|process| {
                        ((process.memory_mb as f64 / mem_total_mb) * 100.0).clamp(0.0, 100.0) as f32
                    }),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(process_search).small().w(px(280.))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .child(render_process_table_header(process_sort, on_cycle_sort, cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(render_process_table(processes.clone(), scroll_handle, cx))
                        .scrollbar(scroll_handle, ScrollbarAxis::Vertical),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Label::new(format!("共 {} 个进程", processes.len()))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
        )
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
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(cx.theme().background)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(render_header_cell(
            "PID",
            px(80.),
            false,
            None,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "名称",
            px(160.),
            false,
            None,
            false,
            Some(ProcessSortColumn::Name),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "命令行",
            px(0.),
            true,
            Some(px(200.)),
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "状态",
            px(80.),
            false,
            None,
            false,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "CPU%",
            px(80.),
            false,
            None,
            true,
            Some(ProcessSortColumn::Cpu),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "内存 (MB)",
            px(100.),
            false,
            None,
            true,
            Some(ProcessSortColumn::Memory),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "虚拟内存 (MB)",
            px(120.),
            false,
            None,
            true,
            None,
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "读取 (MB/s)",
            px(100.),
            false,
            None,
            true,
            Some(ProcessSortColumn::DiskRead),
            sort,
            on_cycle_sort.clone(),
            cx,
        ))
        .child(render_header_cell(
            "写入 (MB/s)",
            px(100.),
            false,
            None,
            true,
            Some(ProcessSortColumn::DiskWrite),
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
    let text = format!("{label}{arrow}");
    let mut cell = div()
        .id(format!("process-header-{label}"))
        .flex_none()
        .h_full()
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
        cell = cell.text_right();
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
    let item_sizes = Rc::new(vec![size(px(0.), px(32.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "process-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    processes
                        .get(index)
                        .map(|process| render_process_row(process, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_process_row<V>(process: &SysProcessInfo, cx: &mut Context<V>) -> impl IntoElement {
    let pid = process.pid;
    h_flex()
        .id(format!("process-row-{}", process.pid))
        .context_menu(move |menu, window, cx| {
            menu.menu("结束任务", Box::new(TerminateProcess { pid }))
                .menu("结束进程树", Box::new(TerminateProcessTree { pid }))
                .menu("打开文件位置", Box::new(RevealProcessExe { pid }))
                .menu("属性", Box::new(ShowProcessDetails { pid }))
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
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(
            div()
                .w(px(80.))
                .flex_none()
                .child(Label::new(process.pid.to_string()).text_sm()),
        )
        .child(
            div()
                .w(px(160.))
                .flex_none()
                .child(Label::new(truncate_text(&process.name, 24)).text_sm()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(200.))
                .child(Label::new(truncate_text(&process.command_line, 48)).text_sm()),
        )
        .child(
            div()
                .w(px(80.))
                .flex_none()
                .child(Label::new(process.status.clone()).text_sm()),
        )
        .child(
            div()
                .w(px(80.))
                .flex_none()
                .text_right()
                .child(Label::new(format!("{:.1}", process.cpu_usage)).text_sm()),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child(Label::new(format!("{}", process.memory_mb)).text_sm()),
        )
        .child(
            div()
                .w(px(120.))
                .flex_none()
                .text_right()
                .child(Label::new(format!("{}", process.virtual_memory_mb)).text_sm()),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child(Label::new(format!("{:.2}", process.disk_read_rate)).text_sm()),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child(Label::new(format!("{:.2}", process.disk_write_rate)).text_sm()),
        )
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

fn helper_row<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .child(
            Label::new(label.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            Label::new(value.to_string())
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground),
        )
}

fn info_item<V>(label: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    div()
        .p(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
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
    card("empty-state")
        .p_4()
        .justify_center()
        .items_center()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            Label::new(message.to_string())
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
}

fn render_services_tab<V: Render>(
    telemetry: &MachineTelemetry,
    service_scroll_handle: &VirtualListScrollHandle,
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
    let running_pct = if service_count > 0 {
        (running as f32 / service_count as f32) * 100.0
    } else {
        0.0
    };
    let stopped_pct = if service_count > 0 {
        (stopped as f32 / service_count as f32) * 100.0
    } else {
        0.0
    };

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "service-count",
                    "服务数",
                    service_count.to_string(),
                    Some(70.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "service-running",
                    "运行中",
                    running.to_string(),
                    Some(running_pct),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "service-stopped",
                    "已停止",
                    stopped.to_string(),
                    Some(stopped_pct),
                    Some(cx.theme().red),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(service_search).small().w(px(280.))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .child(render_service_table_header(cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(render_service_table(
                            services.into(),
                            service_scroll_handle,
                            cx,
                        ))
                        .scrollbar(service_scroll_handle, ScrollbarAxis::Vertical),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Label::new(format!("共 {} 个服务", service_count))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
        )
}

fn render_service_table_header<V: Render>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .id("service-table-header")
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(cx.theme().background)
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(div().w(px(160.)).flex_none().child("名称"))
        .child(div().flex_1().min_w(px(200.)).child("显示名称"))
        .child(div().w(px(100.)).flex_none().child("状态"))
        .child(div().w(px(100.)).flex_none().child("启动类型"))
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
                        .map(|service| render_service_row(service, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_service_row<V: Render>(
    service: &SysServiceInfo,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let name = service.name.clone();
    let status_color = service_status_color(&service.status, cx);
    h_flex()
        .id(format!("service-row-{}", service.name))
        .context_menu(move |menu, _window, _cx| {
            menu.menu("启动", Box::new(StartServiceAction { name: name.clone() }))
                .menu("停止", Box::new(StopServiceAction { name: name.clone() }))
                .menu(
                    "重新启动",
                    Box::new(RestartServiceAction { name: name.clone() }),
                )
        })
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(
            div()
                .w(px(160.))
                .flex_none()
                .child(Label::new(service.name.clone()).text_sm()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(200.))
                .child(Label::new(truncate_text(&service.display_name, 48)).text_sm()),
        )
        .child(
            div().w(px(100.)).flex_none().child(
                Label::new(service.status.clone())
                    .text_sm()
                    .text_color(status_color),
            ),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .child(Label::new(service.start_type.clone()).text_sm()),
        )
}

fn render_users_tab<V: Render>(
    telemetry: &MachineTelemetry,
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

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "user-count",
                    "用户数",
                    users.len().to_string(),
                    Some(50.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "user-admin",
                    "管理员账户",
                    admin_count.to_string(),
                    Some(16.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "user-system",
                    "系统账户",
                    system_count.to_string(),
                    Some(46.0),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(user_search).small().w(px(280.))),
        )
        .child(render_user_table(&users, cx))
}

fn render_user_table<V: Render>(users: &[SysUserInfo], cx: &mut Context<V>) -> impl IntoElement {
    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("名称"))
                    .child(TableHead::new().child("UID"))
                    .child(TableHead::new().child("GID"))
                    .child(TableHead::new().child("所属组")),
            ),
        )
        .child(TableBody::new().children(users.iter().map(|user| {
            TableRow::new()
                .child(TableCell::new().child(user.name.clone()))
                .child(TableCell::new().child(user.uid.clone()))
                .child(TableCell::new().child(user.gid.clone()))
                .child(TableCell::new().child(user.groups.clone()))
        })))
        .child(TableCaption::new().child(format!("共 {} 个用户", users.len())))
        .bg(cx.theme().table)
}

fn service_status_color<V>(status: &str, cx: &Context<V>) -> Hsla {
    match status {
        "运行中" => cx.theme().green,
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

    v_flex()
        .gap(px(14.))
        .size_full()
        .child(
            metric_grid_row()
                .child(render_metric_card(
                    "startup-count",
                    "启动项数",
                    items.len().to_string(),
                    Some(68.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "startup-registry",
                    "注册表项",
                    registry_count.to_string(),
                    Some(64.0),
                    Some(cx.theme().primary),
                    cx,
                ))
                .child(render_metric_card(
                    "startup-folder",
                    "启动文件夹项",
                    folder_count.to_string(),
                    Some(8.0),
                    Some(cx.theme().primary),
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(startup_search).small().w(px(280.))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .child(render_startup_table_header(cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(render_startup_table(&items, scroll_handle, cx))
                        .scrollbar(scroll_handle, ScrollbarAxis::Vertical),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Label::new(format!("共 {} 个启动项", items.len()))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
        )
}

fn render_startup_table_header<V: Render>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .id("startup-table-header")
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .overflow_hidden()
        .bg(cx.theme().background)
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(div().w(px(180.)).flex_none().child("名称"))
        .child(div().flex_1().min_w(px(200.)).child("命令"))
        .child(div().w(px(180.)).flex_none().child("位置"))
}

fn render_startup_table<V: Render>(
    items: &[SysStartupInfo],
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let items = items.to_vec();
    let item_count = items.len().max(1);
    let item_sizes = Rc::new(vec![size(px(0.), px(40.)); item_count]);

    v_virtual_list(
        cx.entity().clone(),
        "startup-virtual-list",
        item_sizes,
        move |_this, visible_range, _window, cx| {
            visible_range
                .filter_map(|index| {
                    items
                        .get(index)
                        .map(|item| render_startup_row(item, cx).into_any_element())
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll_handle)
}

fn render_startup_row<V: Render>(item: &SysStartupInfo, cx: &mut Context<V>) -> impl IntoElement {
    let command = item.command.clone();
    h_flex()
        .id(format!("startup-row-{}", item.name))
        .context_menu(move |menu, _window, _cx| {
            menu.menu(
                "打开文件位置",
                Box::new(RevealStartupItem {
                    command: command.clone(),
                }),
            )
        })
        .w_full()
        .h(px(40.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(
            div()
                .w(px(180.))
                .flex_none()
                .child(Label::new(truncate_text(&item.name, 28)).text_sm()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(200.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(Label::new(truncate_text(&item.command, 80)).text_sm()),
        )
        .child(
            div()
                .w(px(180.))
                .flex_none()
                .child(Label::new(item.location.clone()).text_sm()),
        )
}
