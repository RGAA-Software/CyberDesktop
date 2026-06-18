use std::rc::Rc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::FluentBuilder as _, px, size, AnyElement,
    Context, Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Stateful,
    Styled, Window,
};
use gpui_component::{
    chart::AreaChart,
    h_flex,
    label::Label,
    progress::Progress,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    table::{Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow},
    v_flex, v_virtual_list, ActiveTheme, StyledExt, VirtualListScrollHandle,
};

use crate::monitor_model::{
    bytes_to_gb, bytes_to_mb, chart_ticks, cpu_color, disk_usage_percent,
    format_optional_frequency, format_tick, gpu_key, gpu_memory_percent, network_ipv4,
    sensor_status, MachineTelemetry, MonitorTab,
};
use crate::sys_info::SysProcessInfo;

/// Shared card container used across the dashboard. Callers should still apply
/// `.border_color(cx.theme().border)` and `.bg(cx.theme().secondary)` so the
/// theme is resolved at render time.
fn card(id: impl Into<SharedString>) -> Stateful<gpui::Div> {
    div().id(id.into()).gap_3().p_3().rounded_md().border_1()
}

pub fn render_dashboard<V: Render>(
    telemetry: &MachineTelemetry,
    active_tab: MonitorTab,
    process_scroll_handle: &VirtualListScrollHandle,
    window: &mut Window,
    cx: &mut Context<V>,
) -> AnyElement {
    match active_tab {
        MonitorTab::Overview => render_overview_tab(telemetry, cx).into_any_element(),
        MonitorTab::CpuMemory => render_cpu_memory_tab(telemetry, cx).into_any_element(),
        MonitorTab::Gpu => render_gpu_tab(telemetry, cx).into_any_element(),
        MonitorTab::Storage => render_storage_tab(telemetry, cx).into_any_element(),
        MonitorTab::Network => render_network_tab(telemetry, cx).into_any_element(),
        MonitorTab::Sensors => render_sensors_tab(telemetry, cx).into_any_element(),
        MonitorTab::Processes => {
            render_processes_tab(telemetry, process_scroll_handle, window, cx).into_any_element()
        }
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
        .tick_margin(14);
    if let Some(y_max) = y_max {
        chart = chart
            .y(move |_| y_max)
            .stroke(color.opacity(0.0))
            .fill(cx.theme().background.opacity(0.0));
    }

    v_flex()
        .id(SharedString::from(id.to_string()))
        .min_h(px(260.))
        .gap_3()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
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
                .child(format!(
                    "当前 {:.1} {} | Y轴单位 {} | X轴单位 {}",
                    current_value, unit, unit, x_unit
                )),
        )
        .child(
            h_flex()
                .gap_3()
                .flex_1()
                .child(
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
                .child(div().flex_1().h_full().child(chart)),
        )
        .child(
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
                    cx,
                ))),
        )
}

fn render_cpu_memory_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    let history: Vec<_> = telemetry.history.iter().cloned().collect();
    v_flex()
        .gap_4()
        .p_4()
        .child(
            h_flex()
                .gap_4()
                .flex_wrap()
                .child(
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
                )
                .child(
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
                    "cpu-chart",
                    "CPU 使用率",
                    history.clone(),
                    |point| point.time.clone(),
                    |point| point.cpu_usage,
                    cx.theme().red,
                    "%",
                    "时间",
                    Some(100.0),
                    cx,
                )))
                .child(div().flex_1().min_w(px(320.)).child(render_chart(
                    "mem-chart",
                    "内存已用",
                    history,
                    |point| point.time.clone(),
                    |point| point.mem_used_gb,
                    cx.theme().blue,
                    "GB",
                    "时间",
                    Some(bytes_to_gb(telemetry.current.mem.total)),
                    cx,
                ))),
        )
        .child(
            card("cpu-cores")
                .gap_2()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(section_title("每核心使用率", cx))
                .children(
                    telemetry
                        .current
                        .cpu
                        .cpus
                        .iter()
                        .enumerate()
                        .map(|(index, cpu)| {
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    Label::new(format!("Core {index}"))
                                        .min_w(px(120.))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    Progress::new(SharedString::from(format!("cpu-core-{index}")))
                                        .w_full()
                                        .h_2()
                                        .color(cx.theme().red)
                                        .value(cpu.usage.clamp(0.0, 100.0)),
                                )
                                .child(
                                    Label::new(format!("{:.1}%", cpu.usage))
                                        .min_w(px(64.))
                                        .text_xs()
                                        .text_color(cpu_color(cpu.usage, cx.theme())),
                                )
                        }),
                ),
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
                    cx,
                ))),
        )
        .child(render_network_table(telemetry, cx))
}

fn render_sensors_tab<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    v_flex()
        .gap_4()
        .p_4()
        .when(telemetry.current.components.is_empty(), |this| {
            this.child(empty_state("当前没有温度传感器数据", cx))
        })
        .when(!telemetry.current.components.is_empty(), |this| {
            let max_temp = telemetry
                .current
                .components
                .iter()
                .map(|component| component.temperature)
                .fold(0.0, f32::max);
            this.child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(render_metric_card(
                        "sensor-count",
                        "传感器数量",
                        telemetry.current.components.len().to_string(),
                        None,
                        None,
                        cx,
                    ))
                    .child(render_metric_card(
                        "sensor-max",
                        "最高当前温度",
                        format!("{max_temp:.1} °C"),
                        None,
                        None,
                        cx,
                    )),
            )
            .child(render_sensor_table(telemetry, cx))
        })
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
                    .child(TableHead::new().text_right().child("已用(%)")),
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

fn render_sensor_table<V>(telemetry: &MachineTelemetry, cx: &Context<V>) -> impl IntoElement {
    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("标签"))
                    .child(TableHead::new().text_right().child("当前温度"))
                    .child(TableHead::new().text_right().child("最大"))
                    .child(TableHead::new().text_right().child("临界"))
                    .child(TableHead::new().child("状态")),
            ),
        )
        .child(
            TableBody::new().children(telemetry.current.components.iter().map(|component| {
                TableRow::new()
                    .child(TableCell::new().child(component.label.clone()))
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.1} °C", component.temperature)),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.1} °C", component.max)),
                    )
                    .child(
                        TableCell::new()
                            .text_right()
                            .child(format!("{:.1} °C", component.critical)),
                    )
                    .child(TableCell::new().child(sensor_status(component)))
            })),
        )
        .child(TableCaption::new().child(format!(
            "共 {} 个温度组件",
            telemetry.current.components.len()
        )))
        .bg(cx.theme().table)
}

fn render_processes_tab<V: Render>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    _window: &mut Window,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let top_cpu = telemetry.current.processes.first();
    let top_mem = telemetry
        .current
        .processes
        .iter()
        .max_by_key(|process| process.memory);

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
                    telemetry.current.processes.len().to_string(),
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
            v_flex()
                .flex_1()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .child(render_process_table_header(cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(render_process_table(telemetry, scroll_handle, cx))
                        .scrollbar(scroll_handle, ScrollbarAxis::Vertical),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Label::new(format!("共 {} 个进程", telemetry.current.processes.len()))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                ),
        )
}

fn render_process_table_header<V>(cx: &mut Context<V>) -> impl IntoElement {
    h_flex()
        .h(px(32.))
        .px(px(12.))
        .gap(px(8.))
        .items_center()
        .bg(cx.theme().background)
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(div().w(px(80.)).flex_none().child("PID"))
        .child(div().w(px(160.)).flex_none().child("名称"))
        .child(div().flex_1().min_w(px(200.)).child("命令行"))
        .child(div().w(px(80.)).flex_none().child("状态"))
        .child(div().w(px(80.)).flex_none().text_right().child("CPU%"))
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child("内存 (MB)"),
        )
        .child(
            div()
                .w(px(120.))
                .flex_none()
                .text_right()
                .child("虚拟内存 (MB)"),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child("读取 (MB/s)"),
        )
        .child(
            div()
                .w(px(100.))
                .flex_none()
                .text_right()
                .child("写入 (MB/s)"),
        )
}

fn render_process_table<V: Render>(
    telemetry: &MachineTelemetry,
    scroll_handle: &VirtualListScrollHandle,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let processes = telemetry.current.processes.clone();
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
    h_flex()
        .id(format!("process-row-{}", process.pid))
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
            div().w(px(100.)).flex_none().text_right().child(
                Label::new(format!("{:.2}", bytes_to_mb(process.disk_read_bytes))).text_sm(),
            ),
        )
        .child(
            div().w(px(100.)).flex_none().text_right().child(
                Label::new(format!("{:.2}", bytes_to_mb(process.disk_written_bytes))).text_sm(),
            ),
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
