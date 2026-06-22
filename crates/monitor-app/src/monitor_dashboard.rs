use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, size, AnyElement, App,
    Context, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    chart::AreaChart,
    h_flex,
    input::{Input, InputState},
    label::Label,
    progress::Progress,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    sidebar::{Sidebar, SidebarCollapsible, SidebarMenu, SidebarMenuItem},
    table::{Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow},
    v_flex, v_virtual_list, ActiveTheme, IconName, Sizable, StyledExt, VirtualListScrollHandle,
};

use crate::monitor_actions::{
    RestartServiceAction, ResumeProcess, RevealProcessExe, RevealStartupItem, SetProcessAffinity,
    SetProcessIoPriority, SetProcessPriority, ShowProcessDetails, StartServiceAction,
    StopServiceAction, SuspendProcess, TerminateProcess, TerminateProcessTree,
};
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

fn build_monitor_tab_menu<F>(active_tab: MonitorTab, on_click: F) -> Sidebar<SidebarMenu>
where
    F: Fn(MonitorTab, &mut Window, &mut App) + Clone + 'static,
{
    let tabs: [(MonitorTab, &'static str, IconName); 10] = [
        (MonitorTab::Overview, "总览", IconName::ChartPie),
        (MonitorTab::Cpu, "CPU", IconName::Cpu),
        (MonitorTab::Memory, "内存", IconName::MemoryStick),
        (MonitorTab::Gpu, "GPU", IconName::Frame),
        (MonitorTab::Storage, "存储", IconName::HardDrive),
        (MonitorTab::Network, "网络", IconName::Network),
        (MonitorTab::Processes, "进程", IconName::SquareTerminal),
        (MonitorTab::Services, "服务", IconName::Settings),
        (MonitorTab::Startup, "启动项", IconName::Play),
        (MonitorTab::Users, "用户", IconName::User),
    ];

    Sidebar::new("monitor-tabs")
        .collapsible(SidebarCollapsible::None)
        .w_full()
        .child(SidebarMenu::new().children(tabs.iter().map(|(tab, label, icon)| {
            let tab = *tab;
            let on_click = on_click.clone();
            SidebarMenuItem::new(*label)
                .icon(icon.clone())
                .active(active_tab == tab)
                .on_click(move |_event, window, cx| {
                    on_click(tab, window, cx);
                })
        })))
}

pub fn render_monitor_tab_sidebar<V, F>(active_tab: MonitorTab, on_click: F, cx: &Context<V>) -> impl IntoElement
where
    F: Fn(MonitorTab, &mut Window, &mut App) + Clone + 'static,
{
    div()
        .id("monitor-tab-sidebar")
        .size_full()
        .min_h_0()
        .bg(cx.theme().secondary)
        .border_r_1()
        .border_color(cx.theme().border)
        .p_2()
        .overflow_y_scroll()
        .child(build_monitor_tab_menu(active_tab, on_click))
}

pub fn render_monitor_tab_menu<F>(active_tab: MonitorTab, on_click: F) -> impl IntoElement
where
    F: Fn(MonitorTab, &mut Window, &mut App) + Clone + 'static,
{
    build_monitor_tab_menu(active_tab, on_click)
}

fn render_metric_card<V>(
    id: &str,
    title: &str,
    value: String,
    percent: Option<f32>,
    progress_color: Option<Hsla>,
    cx: &Context<V>,
) -> impl IntoElement {
    card(id)
        .gap_2()
        .flex_1()
        .min_w(px(180.))
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            Label::new(title.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            Label::new(value)
                .text_lg()
                .text_color(cx.theme().foreground),
        )
        .when_some(percent, |this, percent| {
            this.child(
                Progress::new(SharedString::from(format!("{id}-progress")))
                    .w_full()
                    .h_2()
                    .when_some(progress_color, |this, color| this.color(color))
                    .value(percent.clamp(0.0, 100.0)),
            )
        })
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
    let chart_min_h = if compact { px(200.) } else { px(300.) };
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
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_sm()
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
                        .h_full()
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
                                            .children(x_labels.iter().enumerate().map(
                                                |(i, text)| {
                                                    let last = x_label_count - 1;
                                                    div()
                                                        .flex_1()
                                                        .h_full()
                                                        .flex()
                                                        .items_end()
                                                        .when(i == 0, |this| this.justify_start())
                                                        .when(i == last, |this| this.justify_end())
                                                        .when(i > 0 && i < last, |this| {
                                                            this.justify_center()
                                                        })
                                                        .child(
                                                            Label::new(text.clone())
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                ),
                                                        )
                                                },
                                            )),
                                    ),
                            )
                        }),
                ),
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
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
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
                    Some(cx.theme().blue),
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
                    if telemetry.current.gpus.is_empty() {
                        "无 GPU 数据".to_string()
                    } else {
                        format!("{:.0}%", telemetry.primary_gpu_percent())
                    },
                    if telemetry.current.gpus.is_empty() {
                        None
                    } else {
                        Some(telemetry.primary_gpu_percent())
                    },
                    Some(cx.theme().green),
                    cx,
                ))
                .child(render_metric_card(
                    "overview-send",
                    "主网卡发送速率",
                    format!("{:.2} MB/s", telemetry.latest_send_rate()),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "overview-recv",
                    "主网卡接收速率",
                    format!("{:.2} MB/s", telemetry.latest_recv_rate()),
                    None,
                    None,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_4()
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
                    cx.theme().blue,
                    "%",
                    "时间",
                    Some(100.0),
                    true,
                    true,
                    cx,
                ))),
        )
        .when(!telemetry.current.gpus.is_empty(), |this| {
            this.child(
                v_flex().gap_3().child(section_title("GPU 总览", cx)).child(
                    v_flex()
                        .gap_4()
                        .children(telemetry.current.gpus.iter().map(|gpu| {
                            let gpu_id = gpu_key(gpu);
                            let gpu_name = gpu.brand.clone();
                            let mem_total_gb = gpu.mem_total_gb as f64;
                            let history = telemetry
                                .gpu_history
                                .get(&gpu_id)
                                .map(|items| items.iter().cloned().collect::<Vec<_>>())
                                .unwrap_or_default();

                            h_flex()
                                .gap_4()
                                .flex_wrap()
                                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                                    &format!("overview-gpu-chart-usage-{gpu_id}"),
                                    &format!("{gpu_name} GPU 利用率"),
                                    history.clone(),
                                    |point| point.time.clone(),
                                    |point| point.usage,
                                    cx.theme().blue,
                                    "%",
                                    "时间",
                                    Some(100.0),
                                    true,
                                    true,
                                    cx,
                                )))
                                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                                    &format!("overview-gpu-chart-memory-{gpu_id}"),
                                    &format!("{gpu_name} 显存利用率"),
                                    history,
                                    |point| point.time.clone(),
                                    move |point| {
                                        if mem_total_gb <= 0.0 {
                                            0.0
                                        } else {
                                            (point.memory_used_gb / mem_total_gb) * 100.0
                                        }
                                    },
                                    cx.theme().green,
                                    "%",
                                    "时间",
                                    Some(100.0),
                                    true,
                                    true,
                                    cx,
                                )))
                        })),
                ),
            )
        })
        .child(
            h_flex()
                .gap_4()
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "overview-chart-send",
                    "网络发送速率",
                    history.clone(),
                    |point| point.time.clone(),
                    |point| point.net_send_mb,
                    cx.theme().green,
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
        .child(div().h(px(15.)))
}

fn render_cpu_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex().gap_4().flex_wrap().child(
                card("cpu-info")
                    .flex_1()
                    .min_w(px(320.))
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(section_title("CPU 概览", cx))
                    .child(kv_line("型号", &telemetry.current.cpu.brand, cx))
                    .child(kv_line("Vendor", &telemetry.current.cpu.vendor, cx))
                    .child(kv_line(
                        "基准频率",
                        &format!("{:.2} GHz", telemetry.current.cpu.base_frequency),
                        cx,
                    ))
                    .child(kv_line(
                        "最大频率",
                        &format!("{:.2} GHz", telemetry.current.cpu.max_frequency),
                        cx,
                    ))
                    .child(kv_line(
                        "当前频率",
                        &format_optional_frequency(telemetry.current.cpu.current_frequency),
                        cx,
                    ))
                    .child(kv_line(
                        "逻辑核心数",
                        &telemetry.current.cpu.cpus.len().to_string(),
                        cx,
                    ))
                    .child(kv_line(
                        "总使用率",
                        &format!("{:.1}%", telemetry.current.cpu.usage),
                        cx,
                    )),
            ),
        )
        .child(div().min_h(px(260.)).child(render_chart(
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
        )))
        .children(
            (0..telemetry.current.cpu.cpus.len())
                .step_by(5)
                .map(|start| {
                    h_flex()
                        .gap_4()
                        .min_h(px(200.))
                        .children((0..5).map(|offset| {
                            let index = start + offset;
                            if index >= telemetry.current.cpu.cpus.len() {
                                return div().flex_1().min_h(px(200.)).into_any_element();
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
                                .min_h(px(200.))
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
        .child(div().h(px(15.)))
}

fn render_memory_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex().gap_4().flex_wrap().child(
                card("mem-info")
                    .flex_1()
                    .min_w(px(320.))
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(section_title("内存概览", cx))
                    .child(kv_line(
                        "总内存",
                        &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.total)),
                        cx,
                    ))
                    .child(kv_line(
                        "已用内存",
                        &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.used)),
                        cx,
                    ))
                    .child(kv_line(
                        "可用内存",
                        &format!("{:.1} GB", bytes_to_gb(telemetry.current.mem.available)),
                        cx,
                    ))
                    .child(kv_line(
                        "使用率",
                        &format!("{:.1}%", telemetry.latest_mem_percent()),
                        cx,
                    ))
                    .child(
                        Progress::new("mem-total-progress")
                            .w_full()
                            .h_2()
                            .color(cx.theme().blue)
                            .value(telemetry.latest_mem_percent()),
                    ),
            ),
        )
        .child(
            h_flex()
                .gap_4()
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "mem-chart",
                    "内存已用",
                    history.clone(),
                    |point| point.time.clone(),
                    |point| point.mem_used_gb,
                    cx.theme().blue,
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
                    cx.theme().blue,
                    "%",
                    "时间",
                    Some(100.0),
                    true,
                    true,
                    cx,
                ))),
        )
}

fn render_gpu_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap_4()
        .p_4()
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

            card(format!("gpu-panel-{gpu_id}"))
                .gap_4()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(section_title(&gpu.brand, cx))
                .child(
                    h_flex()
                        .gap_3()
                        .flex_wrap()
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-usage"),
                            "利用率",
                            format!("{}%", gpu.gpu_utilization),
                            Some(gpu.gpu_utilization as f32),
                            Some(cx.theme().blue),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-temp"),
                            "温度",
                            format!("{} °C", gpu.temperature),
                            None,
                            None,
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-fan"),
                            "风扇转速",
                            format!("{} RPM", gpu.fan_speed),
                            None,
                            None,
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-vram"),
                            "显存",
                            format!("{:.2} / {:.2} GB", gpu.mem_used_gb, gpu.mem_total_gb),
                            Some(gpu_memory_percent(gpu)),
                            Some(cx.theme().green),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-encoder"),
                            "编码器",
                            format!("{}%", gpu.encoder_utilization),
                            Some(gpu.encoder_utilization as f32),
                            Some(cx.theme().green),
                            cx,
                        ))
                        .child(render_metric_card(
                            &format!("gpu-{gpu_id}-power"),
                            "功耗上限",
                            format!("{:.1} W", gpu.power_limit as f64 / 1000.0),
                            None,
                            None,
                            cx,
                        )),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(div().flex_1().min_w(px(280.)).child(render_chart(
                            &format!("gpu-{gpu_id}-chart-usage"),
                            "GPU 利用率",
                            history.clone(),
                            |point| point.time.clone(),
                            |point| point.usage,
                            cx.theme().blue,
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
                            cx.theme().green,
                            "GB",
                            "时间",
                            Some(gpu.mem_total_gb as f64),
                            true,
                            true,
                            cx,
                        ))),
                )
                .child(kv_line("唯一标识", &gpu.id, cx))
        }))
}

fn render_storage_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(render_metric_card(
                    "storage-disk-count",
                    "磁盘数量",
                    telemetry.current.disks.len().to_string(),
                    None,
                    None,
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
                    format!("{:.2} MB/s", telemetry.latest_disk_read_rate()),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "storage-write-rate",
                    "磁盘写入速率",
                    format!("{:.2} MB/s", telemetry.latest_disk_write_rate()),
                    None,
                    None,
                    cx,
                )),
        )
        .child(render_disk_table(telemetry, cx))
}

fn render_network_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    let primary = telemetry.current.networks.first();
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(render_metric_card(
                    "network-primary-name",
                    "当前主网卡",
                    primary
                        .map(|item| item.name.clone())
                        .unwrap_or_else(|| "未识别".to_string()),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "network-send-rate",
                    "发送速率",
                    format!("{:.2} MB/s", telemetry.latest_send_rate()),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "network-recv-rate",
                    "接收速率",
                    format!("{:.2} MB/s", telemetry.latest_recv_rate()),
                    None,
                    None,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_4()
                .flex_wrap()
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "network-chart-send",
                    "发送速率",
                    history.clone(),
                    |point| point.time.clone(),
                    |point| point.net_send_mb,
                    cx.theme().green,
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

    v_flex()
        .gap_4()
        .p_4()
        .size_full()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(render_metric_card(
                    "process-count",
                    "进程数",
                    processes.len().to_string(),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "process-top-cpu",
                    "最高 CPU",
                    top_cpu
                        .map(|process| format!("{} {:.1}%", process.name, process.cpu_usage))
                        .unwrap_or_else(|| "-".to_string()),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "process-top-mem",
                    "最高内存",
                    top_mem
                        .map(|process| format!("{} {} MB", process.name, process.memory_mb))
                        .unwrap_or_else(|| "-".to_string()),
                    None,
                    None,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(process_search).small().w(px(220.))),
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

fn section_title<V>(title: &str, cx: &Context<V>) -> impl IntoElement {
    Label::new(title.to_string())
        .text_sm()
        .font_semibold()
        .text_color(cx.theme().foreground)
}

fn kv_line<V>(key: &str, value: &str, cx: &Context<V>) -> impl IntoElement {
    h_flex()
        .justify_between()
        .gap_3()
        .child(
            Label::new(key.to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            Label::new(value.to_string())
                .text_xs()
                .text_color(cx.theme().foreground),
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

    v_flex()
        .gap_4()
        .p_4()
        .size_full()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(render_metric_card(
                    "service-count",
                    "服务数",
                    services.len().to_string(),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "service-running",
                    "运行中",
                    running.to_string(),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "service-stopped",
                    "已停止",
                    stopped.to_string(),
                    None,
                    None,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(service_search).small().w(px(220.))),
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

    v_flex()
        .gap_4()
        .p_4()
        .size_full()
        .child(h_flex().gap_3().flex_wrap().child(render_metric_card(
            "user-count",
            "用户数",
            users.len().to_string(),
            None,
            None,
            cx,
        )))
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(user_search).small().w(px(220.))),
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
        .gap_4()
        .p_4()
        .size_full()
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(render_metric_card(
                    "startup-count",
                    "启动项数",
                    items.len().to_string(),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "startup-registry",
                    "注册表项",
                    registry_count.to_string(),
                    None,
                    None,
                    cx,
                ))
                .child(render_metric_card(
                    "startup-folder",
                    "启动文件夹项",
                    folder_count.to_string(),
                    None,
                    None,
                    cx,
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Input::new(startup_search).small().w(px(220.))),
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
