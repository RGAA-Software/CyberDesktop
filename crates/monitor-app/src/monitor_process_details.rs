use gpui::{
    px, size, App, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, WindowBounds, WindowOptions,
};
use gpui_component::{
    h_flex, label::Label, scroll::ScrollableElement, v_flex, ActiveTheme, StyledExt,
};

use crate::monitor_model::bytes_to_mb;
use crate::sys_info::SysProcessInfo;

pub struct ProcessDetailsView {
    process: SysProcessInfo,
}

impl ProcessDetailsView {
    pub fn open(process: SysProcessInfo, cx: &mut App) {
        let window_options = WindowOptions {
            titlebar: Some(app_ui::TitleBar::title_bar_options()),
            window_bounds: Some(WindowBounds::centered(size(px(520.), px(420.)), cx)),
            ..Default::default()
        };
        let name = process.name.clone();
        let _ = cx.open_window(window_options, move |window, cx| {
            window.set_window_title(&format!("进程属性 - {name}"));
            cx.new(|_cx| ProcessDetailsView {
                process: process.clone(),
            })
        });
    }
}

impl Render for ProcessDetailsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.process;
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                app_ui::TitleBar::new()
                    .h(px(35.))
                    .bg(cx.theme().title_bar)
                    .child(
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
                    .child(kv_line("名称", &p.name, cx))
                    .child(kv_line("可执行文件", &p.exe, cx))
                    .child(kv_line("命令行", &p.command_line, cx))
                    .child(kv_line("状态", &p.status, cx))
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
                    )),
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
