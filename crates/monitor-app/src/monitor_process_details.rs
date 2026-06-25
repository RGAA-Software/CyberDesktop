use gpui::{
    px, size, App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, WindowBounds, WindowOptions,
};
use gpui_component::{
    h_flex, label::Label, scroll::ScrollableElement, table::Table, v_flex, ActiveTheme, StyledExt,
};

use crate::monitor_model::bytes_to_mb;
use crate::monitor_process_detail::ProcessDetailInfo;
use crate::sys_info::SysProcessInfo;

pub struct ProcessDetailsView {
    process: SysProcessInfo,
    details: ProcessDetailInfo,
}

impl ProcessDetailsView {
    pub fn open(process: SysProcessInfo, details: ProcessDetailInfo, cx: &mut App) {
        let window_options = WindowOptions {
            titlebar: Some(app_ui::TitleBar::title_bar_options()),
            window_bounds: Some(WindowBounds::centered(size(px(720.), px(580.)), cx)),
            ..Default::default()
        };
        let name = process.name.clone();
        let _ = cx.open_window(window_options, move |window, cx| {
            window.set_window_title(&format!("进程属性 - {name}"));
            cx.new(|_cx| ProcessDetailsView {
                process: process.clone(),
                details: details.clone(),
            })
        });
    }
}

impl Render for ProcessDetailsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.process;
        let d = &self.details;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                app_ui::TitleBar::new().bg(cx.theme().title_bar).child(
                    h_flex()
                        .id("process-details-title")
                        .h_full()
                        .w_full()
                        .items_center()
                        .px(px(12.))
                        .child(
                            Label::new(format!("进程属性 - {}", p.name))
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground),
                        ),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .p_4()
                    .gap_3()
                    .child(section_title("General", cx))
                    .child(kv_line("PID", &p.pid.to_string(), cx))
                    .child(kv_line("父 PID", &p.parent_pid.to_string(), cx))
                    .child(kv_line("名称", &p.name, cx))
                    .child(kv_line("可执行文件", &p.exe, cx))
                    .child(kv_line("命令行", &p.command_line, cx))
                    .child(kv_line("状态", &p.status, cx))
                    .child(kv_line("网络连接数", &d.network.len().to_string(), cx))
                    .child(section_title("Performance", cx))
                    .child(kv_line("CPU 使用率", &format!("{:.1}%", p.cpu_usage), cx))
                    .child(kv_line("物理内存", &format!("{} MB", p.memory_mb), cx))
                    .child(kv_line(
                        "虚拟内存",
                        &format!("{} MB", p.virtual_memory_mb),
                        cx,
                    ))
                    .child(kv_line(
                        "磁盘读取",
                        &format!("{:.2} MB", bytes_to_mb(p.disk_read_bytes)),
                        cx,
                    ))
                    .child(kv_line(
                        "磁盘写入",
                        &format!("{:.2} MB", bytes_to_mb(p.disk_written_bytes)),
                        cx,
                    ))
                    .child(section_title("Token", cx))
                    .child(kv_line("用户", &d.token_user, cx))
                    .child(section_title("Handles", cx))
                    .child(kv_line("句柄数", &d.handle_count.to_string(), cx))
                    .child(section_title("Threads", cx))
                    .child(render_threads_table(d, cx))
                    .child(section_title("Modules", cx))
                    .child(render_modules_table(d, cx))
                    .child(section_title("Network", cx))
                    .child(render_network_table(d, cx))
                    .child(section_title("Environment", cx))
                    .children(
                        d.environment
                            .iter()
                            .map(|(key, value)| kv_line(key, value, cx)),
                    ),
            )
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

fn render_threads_table<V>(details: &ProcessDetailInfo, cx: &Context<V>) -> impl IntoElement {
    use gpui_component::table::{TableBody, TableCell, TableHead, TableHeader, TableRow};

    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("TID"))
                    .child(TableHead::new().child("开始地址")),
            ),
        )
        .child(
            TableBody::new().children(details.threads.iter().map(|thread| {
                TableRow::new()
                    .child(TableCell::new().child(thread.tid.to_string()))
                    .child(TableCell::new().child(thread.start_address.clone()))
            })),
        )
        .bg(cx.theme().table)
}

fn render_modules_table<V>(details: &ProcessDetailInfo, cx: &Context<V>) -> impl IntoElement {
    use gpui_component::table::{TableBody, TableCell, TableHead, TableHeader, TableRow};

    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("名称"))
                    .child(TableHead::new().child("路径"))
                    .child(TableHead::new().child("基址"))
                    .child(TableHead::new().child("大小")),
            ),
        )
        .child(
            TableBody::new().children(details.modules.iter().map(|module| {
                TableRow::new()
                    .child(TableCell::new().child(module.name.clone()))
                    .child(TableCell::new().child(module.path.clone()))
                    .child(TableCell::new().child(module.base_address.clone()))
                    .child(TableCell::new().child(module.size.clone()))
            })),
        )
        .bg(cx.theme().table)
}

fn render_network_table<V>(details: &ProcessDetailInfo, cx: &Context<V>) -> impl IntoElement {
    use gpui_component::table::{TableBody, TableCell, TableHead, TableHeader, TableRow};

    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().child("协议"))
                    .child(TableHead::new().child("本地地址"))
                    .child(TableHead::new().child("远程地址"))
                    .child(TableHead::new().child("状态"))
                    .child(TableHead::new().child("PID")),
            ),
        )
        .child(
            TableBody::new().children(details.network.iter().map(|conn| {
                TableRow::new()
                    .child(TableCell::new().child(conn.protocol.clone()))
                    .child(TableCell::new().child(conn.local.clone()))
                    .child(TableCell::new().child(conn.remote.clone()))
                    .child(TableCell::new().child(conn.state.clone()))
                    .child(TableCell::new().child(conn.pid.to_string()))
            })),
        )
        .bg(cx.theme().table)
}
