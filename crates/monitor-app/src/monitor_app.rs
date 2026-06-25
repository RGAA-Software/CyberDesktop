use std::time::Duration;

use gpui::{
    div, px, size, App, AppContext, ClipboardItem, Context, Entity,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, WindowBounds, WindowOptions,
};
use gpui_component::{
    h_flex, input::InputState, scroll::ScrollableElement as _, v_flex, ActiveTheme,
    Root, ThemeMode, VirtualListScrollHandle,
};
use smol::Timer;

use files_core::{init_tracing, set_config_app_id, MONITOR_CONFIG_APP_ID};

use crate::monitor_actions::{
    CopyProcessInfo, CycleProcessSort, ProcessActionHandler, RestartServiceAction, ResumeProcess,
    RevealProcessExe, RevealStartupItem, SetProcessAffinity, SetProcessIoPriority,
    SetProcessPriority, ShowProcessDetails, StartServiceAction, StopServiceAction, SuspendProcess,
    TerminateProcess, TerminateProcessTree,
};
use crate::monitor_codec::encode_telemetry;
use crate::monitor_dashboard::{
    monitor_title_crumb, render_dashboard, render_monitor_client_sidebar, tab_manages_bottom_padding,
    topbar_icon_button, MONITOR_MAIN_TITLE_BAR_HEIGHT,
};
use crate::monitor_icons;
use crate::monitor_model::{
    MachineTelemetry, MonitorTab, ProcessSort, ProcessSortColumn, SortDirection,
};
use crate::monitor_process_ctrl;
use crate::monitor_process_details::ProcessDetailsView;
use crate::monitor_sender::MonitorSenderHandle;
use crate::monitor_settings::{
    build_monitor_settings, init_monitor_connection, load_monitor_connection_config,
};
use crate::sys_info::{SysInfo, SysProcessInfo};
use crate::sys_info_mgr::SysInfoWorker;
use crate::tray::{self, TrayCommand};

const INTERVAL: Duration = Duration::from_secs(1);
const COLLECT_POLL: Duration = Duration::from_millis(16);

/// Runs `SysInfoWorker::collect` off the GPUI main thread (smol executor, no Tokio runtime).
async fn collect_sysinfo_async(worker: SysInfoWorker) -> Option<SysInfo> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = tx.send(worker.collect());
    });
    loop {
        match rx.try_recv() {
            Ok(snapshot) => return Some(snapshot),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                Timer::after(COLLECT_POLL).await;
            }
        }
    }
}

pub struct SysMonitorApp {
    sysinfo: SysInfoWorker,
    telemetry: MachineTelemetry,
    active_tab: MonitorTab,
    sender: MonitorSenderHandle,
    process_scroll: VirtualListScrollHandle,
    process_h_scroll: VirtualListScrollHandle,
    process_search: Entity<InputState>,
    process_sort: ProcessSort,
    service_scroll: VirtualListScrollHandle,
    service_h_scroll: VirtualListScrollHandle,
    service_search: Entity<InputState>,
    startup_scroll: VirtualListScrollHandle,
    startup_h_scroll: VirtualListScrollHandle,
    startup_search: Entity<InputState>,
    user_scroll: VirtualListScrollHandle,
    user_h_scroll: VirtualListScrollHandle,
    user_search: Entity<InputState>,
}

impl ProcessActionHandler for SysMonitorApp {
    fn terminate_process(&mut self, pid: u32, cx: &mut Context<Self>) {
        if self.sysinfo.kill_process(pid) {
            cx.notify();
        }
    }

    fn reveal_process_exe(&mut self, pid: u32, _cx: &mut Context<Self>) {
        if let Some(process) = self
            .telemetry
            .current
            .processes
            .iter()
            .find(|p| p.pid == pid)
        {
            let path = resolve_process_exe_path(process);
            if !path.is_empty() {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{path}"))
                    .spawn();
            }
        }
    }

    fn start_service(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let ok = self.sysinfo.start_service(name);
        if ok {
            cx.notify();
        }
        ok
    }

    fn stop_service(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let ok = self.sysinfo.stop_service(name);
        if ok {
            cx.notify();
        }
        ok
    }

    fn restart_service(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        if self.sysinfo.stop_service(name) {
            let ok = self.sysinfo.start_service(name);
            if ok {
                cx.notify();
            }
            return ok;
        }
        false
    }

    fn reveal_startup_item(&mut self, command: &str, _cx: &mut Context<Self>) {
        let path = resolve_startup_command_path(command);
        if !path.is_empty() {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{path}"))
                .spawn();
        }
    }

    fn set_process_priority(&mut self, pid: u32, priority: &str, cx: &mut Context<Self>) -> bool {
        let ok = monitor_process_ctrl::set_process_priority(pid, priority);
        if ok {
            cx.notify();
        }
        ok
    }

    fn set_process_io_priority(
        &mut self,
        pid: u32,
        priority: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let ok = monitor_process_ctrl::set_process_io_priority(pid, priority);
        if ok {
            cx.notify();
        }
        ok
    }

    fn set_process_affinity(&mut self, pid: u32, mask: u64, cx: &mut Context<Self>) -> bool {
        let ok = monitor_process_ctrl::set_process_affinity(pid, mask);
        if ok {
            cx.notify();
        }
        ok
    }

    fn suspend_process(&mut self, pid: u32, cx: &mut Context<Self>) -> bool {
        let ok = monitor_process_ctrl::suspend_process(pid);
        if ok {
            cx.notify();
        }
        ok
    }

    fn resume_process(&mut self, pid: u32, cx: &mut Context<Self>) -> bool {
        let ok = monitor_process_ctrl::resume_process(pid);
        if ok {
            cx.notify();
        }
        ok
    }

    fn terminate_process_tree(&mut self, pid: u32, cx: &mut Context<Self>) -> bool {
        let processes: Vec<(u32, Option<u32>)> = self
            .telemetry
            .current
            .processes
            .iter()
            .map(|p| {
                (
                    p.pid,
                    if p.parent_pid == 0 {
                        None
                    } else {
                        Some(p.parent_pid)
                    },
                )
            })
            .collect();
        let ok = monitor_process_ctrl::terminate_process_tree(pid, &processes);
        if ok {
            cx.notify();
        }
        ok
    }
}

impl SysMonitorApp {
    fn new(_window: &mut Window, cx: &mut Context<Self>, sysinfo: SysInfoWorker) -> Self {
        let telemetry = MachineTelemetry::new(Default::default());
        let connection_config = load_monitor_connection_config();
        let sender = MonitorSenderHandle::new();
        init_monitor_connection(cx, sender.clone(), connection_config);

        let process_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索进程..."));
        let service_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索服务..."));
        let startup_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索启动项..."));
        let user_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索用户..."));
        let this = Self {
            sysinfo,
            telemetry,
            active_tab: MonitorTab::Overview,
            sender,
            process_scroll: VirtualListScrollHandle::new(),
            process_h_scroll: VirtualListScrollHandle::new(),
            process_search,
            process_sort: ProcessSort::default(),
            service_scroll: VirtualListScrollHandle::new(),
            service_h_scroll: VirtualListScrollHandle::new(),
            service_search,
            startup_scroll: VirtualListScrollHandle::new(),
            startup_h_scroll: VirtualListScrollHandle::new(),
            startup_search,
            user_scroll: VirtualListScrollHandle::new(),
            user_h_scroll: VirtualListScrollHandle::new(),
            user_search,
        };

        let worker = this.sysinfo.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let worker = worker.clone();
                let snapshot = match collect_sysinfo_async(worker).await {
                    Some(snapshot) => snapshot,
                    None => break,
                };
                let payload = encode_telemetry(&snapshot).unwrap_or_default();
                if this
                    .update(cx, |this, cx| {
                        this.telemetry.apply_snapshot(snapshot);
                        this.sender.set_latest_payload(payload);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                Timer::after(INTERVAL).await;
            }
        })
        .detach();

        this
    }
}

fn monitor_tab_title(tab: MonitorTab) -> &'static str {
    match tab {
        MonitorTab::Overview => "总览",
        MonitorTab::Cpu => "CPU",
        MonitorTab::Memory => "内存",
        MonitorTab::Gpu => "GPU",
        MonitorTab::Storage => "存储",
        MonitorTab::Network => "网络",
        MonitorTab::Processes => "进程",
        MonitorTab::Services => "服务",
        MonitorTab::Startup => "启动项",
        MonitorTab::Users => "用户",
    }
}

fn monitor_tab_subtitle(tab: MonitorTab) -> &'static str {
    match tab {
        MonitorTab::Overview => "关键资源趋势与系统健康概览",
        MonitorTab::Cpu => "CPU 总览、总使用率与逻辑核心负载",
        MonitorTab::Memory => "内存容量、已用空间和使用率趋势",
        MonitorTab::Gpu => "显卡负载、温度、显存与风扇信息",
        MonitorTab::Storage => "磁盘容量、占用率与读写吞吐",
        MonitorTab::Network => "网卡速率、累计流量与连接信息",
        MonitorTab::Processes => "进程列表、CPU/内存/IO 排序与搜索",
        MonitorTab::Services => "Windows 服务状态与启动类型",
        MonitorTab::Startup => "注册表与启动文件夹中的开机项",
        MonitorTab::Users => "本机用户、SID 与用户组信息",
    }
}

impl Render for SysMonitorApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let tab_view = cx.entity().clone();
        let is_dark = cx.theme().mode.is_dark();
        h_flex()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(Self::on_terminate_process))
            .on_action(cx.listener(Self::on_reveal_process_exe))
            .on_action(cx.listener(Self::on_show_process_details))
            .on_action(cx.listener(Self::on_copy_process_info))
            .on_action(cx.listener(Self::on_start_service))
            .on_action(cx.listener(Self::on_stop_service))
            .on_action(cx.listener(Self::on_restart_service))
            .on_action(cx.listener(Self::on_reveal_startup_item))
            .on_action(cx.listener(Self::on_set_process_priority))
            .on_action(cx.listener(Self::on_set_process_io_priority))
            .on_action(cx.listener(Self::on_set_process_affinity))
            .on_action(cx.listener(Self::on_suspend_process))
            .on_action(cx.listener(Self::on_resume_process))
            .on_action(cx.listener(Self::on_terminate_process_tree))
            .child(render_monitor_client_sidebar(
                self.active_tab,
                move |tab, _window, cx| {
                    let _ = tab_view.update(cx, |this, cx| {
                        this.active_tab = tab;
                        cx.notify();
                    });
                },
                cx,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        app_ui::TitleBar::new()
                            .design_window_controls(true)
                            .h(MONITOR_MAIN_TITLE_BAR_HEIGHT)
                            .bg(cx.theme().title_bar)
                            .border_b_1()
                            .border_color(cx.theme().title_bar_border)
                            .child(
                                h_flex()
                                    .id("title-bar-inner")
                                    .h_full()
                                    .w_full()
                                    .min_w_0()
                                    .flex_1()
                                    .items_center()
                                    .pl(px(24.))
                                    .pr(px(18.))
                                    .child(monitor_title_crumb(
                                        monitor_tab_title(self.active_tab),
                                        monitor_tab_subtitle(self.active_tab),
                                        cx,
                                    )),
                            )
                            .trailing_before_controls(
                                h_flex()
                                    .id("title-bar-actions")
                                    .h_full()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        topbar_icon_button(
                                            "monitor-refresh",
                                            monitor_icons::REFRESH,
                                            &*cx,
                                        )
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| cx.notify(view.entity_id())
                                        }),
                                    )
                                    .child(
                                        topbar_icon_button(
                                            "monitor-theme-toggle",
                                            if is_dark {
                                                monitor_icons::SUN
                                            } else {
                                                monitor_icons::MOON
                                            },
                                            &*cx,
                                        )
                                        .on_click(|_, _, cx| {
                                            let mode = if cx.theme().mode.is_dark() {
                                                ThemeMode::Light
                                            } else {
                                                ThemeMode::Dark
                                            };
                                            app_ui::apply_theme_mode(mode, cx);
                                        }),
                                    )
                                    .child(
                                        topbar_icon_button(
                                            "monitor-settings",
                                            monitor_icons::SETTINGS,
                                            &*cx,
                                        )
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|_this, _e, _w, cx| {
                                                cx.stop_propagation();
                                                app_ui::SettingsWindowState::open_with(
                                                    cx,
                                                    |cx| build_monitor_settings(cx),
                                                    None,
                                                );
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child({
                        let is_list_tab = tab_manages_bottom_padding(self.active_tab);
                        let bottom_pad = if is_list_tab { px(0.) } else { px(30.) };
                        let dashboard = render_dashboard(
                            &self.telemetry,
                            self.active_tab,
                            &self.process_scroll,
                            &self.process_h_scroll,
                            &self.process_search,
                            self.process_sort,
                            &self.service_scroll,
                            &self.service_h_scroll,
                            &self.service_search,
                            &self.startup_scroll,
                            &self.startup_h_scroll,
                            &self.startup_search,
                            &self.user_scroll,
                            &self.user_h_scroll,
                            &self.user_search,
                            move |column, window, cx| {
                                view.update(cx, |this, cx| {
                                    this.on_cycle_process_sort(
                                        &CycleProcessSort { column },
                                        window,
                                        cx,
                                    );
                                });
                            },
                            _window,
                            cx,
                        );

                        if is_list_tab {
                            // 进程/服务/启动项/用户 使用自己的 virtual list 滚动条，
                            // 保持原来的外层滚动容器不变。
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .px(px(24.))
                                .pt(px(20.))
                                .pb(bottom_pad)
                                .child(
                                    div()
                                        .size_full()
                                        .overflow_y_scrollbar()
                                        .child(dashboard),
                                )
                        } else {
                            // 总览/CPU/GPU/存储/网络：滚动条贴右侧窗口，内容加内边距避免遮挡。
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(
                                    div()
                                        .size_full()
                                        .overflow_y_scrollbar()
                                        .child(
                                            div()
                                                .px(px(24.))
                                                .pt(px(20.))
                                                .pb(bottom_pad)
                                                .child(dashboard)
                                                .child(div().h(px(15.))),
                                        ),
                                )
                        }
                    }),
            )
    }
}

impl SysMonitorApp {
    fn on_terminate_process(
        &mut self,
        action: &TerminateProcess,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminate_process(action.pid, cx);
    }

    fn on_reveal_process_exe(
        &mut self,
        action: &RevealProcessExe,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_process_exe(action.pid, cx);
    }

    fn on_show_process_details(
        &mut self,
        action: &ShowProcessDetails,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(process) = self
            .telemetry
            .current
            .processes
            .iter()
            .find(|p| p.pid == action.pid)
        {
            let details = self
                .sysinfo
                .get_process_details(action.pid)
                .unwrap_or_default();
            ProcessDetailsView::open(process.clone(), details, cx);
        }
    }

    fn on_copy_process_info(
        &mut self,
        action: &CopyProcessInfo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copy_process_info(action.pid, cx);
    }

    fn on_cycle_process_sort(
        &mut self,
        action: &CycleProcessSort,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.process_sort.column == action.column {
            self.process_sort.direction = self.process_sort.direction.toggle();
        } else {
            let default_direction = match action.column {
                ProcessSortColumn::Name => SortDirection::Asc,
                _ => SortDirection::Desc,
            };
            self.process_sort = ProcessSort {
                column: action.column,
                direction: default_direction,
            };
        }
        cx.notify();
    }

    fn on_start_service(
        &mut self,
        action: &StartServiceAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_service(&action.name, cx);
    }

    fn on_stop_service(
        &mut self,
        action: &StopServiceAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_service(&action.name, cx);
    }

    fn on_restart_service(
        &mut self,
        action: &RestartServiceAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.restart_service(&action.name, cx);
    }

    fn on_reveal_startup_item(
        &mut self,
        action: &RevealStartupItem,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_startup_item(&action.command, cx);
    }

    fn on_set_process_priority(
        &mut self,
        action: &SetProcessPriority,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_process_priority(action.pid, &action.priority, cx);
    }

    fn on_set_process_io_priority(
        &mut self,
        action: &SetProcessIoPriority,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_process_io_priority(action.pid, &action.priority, cx);
    }

    fn on_set_process_affinity(
        &mut self,
        action: &SetProcessAffinity,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_process_affinity(action.pid, action.affinity_mask, cx);
    }

    fn on_suspend_process(
        &mut self,
        action: &SuspendProcess,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suspend_process(action.pid, cx);
    }

    fn on_resume_process(
        &mut self,
        action: &ResumeProcess,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.resume_process(action.pid, cx);
    }

    fn on_terminate_process_tree(
        &mut self,
        action: &TerminateProcessTree,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminate_process_tree(action.pid, cx);
    }

    fn copy_process_info(&mut self, pid: u32, cx: &mut Context<Self>) {
        if let Some(process) = self
            .telemetry
            .current
            .processes
            .iter()
            .find(|p| p.pid == pid)
        {
            if let Ok(json) = serde_json::to_string_pretty(process) {
                cx.write_to_clipboard(ClipboardItem::new_string(json));
            }
        }
    }
}

fn resolve_process_exe_path(process: &SysProcessInfo) -> String {
    use std::path::Path;

    if !process.exe.is_empty() && Path::new(&process.exe).exists() {
        return process.exe.clone();
    }

    if let Some(path) = extract_first_path_from_command_line(&process.command_line) {
        if Path::new(&path).exists() {
            return path;
        }
    }

    String::new()
}

fn extract_first_path_from_command_line(command_line: &str) -> Option<String> {
    let trimmed = command_line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('"') {
        let rest = &trimmed[1..];
        rest.find('"').map(|index| rest[..index].to_string())
    } else {
        trimmed
            .split_whitespace()
            .next()
            .map(|token| token.to_string())
    }
}

fn resolve_startup_command_path(command: &str) -> String {
    use std::path::Path;

    if command.to_lowercase().ends_with(".lnk") && Path::new(command).exists() {
        return command.to_string();
    }

    if let Some(path) = extract_first_path_from_command_line(command) {
        if Path::new(&path).exists() {
            return path;
        }
    }

    String::new()
}

pub fn run(start_hidden: bool) {
    tray::init_tray("CyberMonitor");
    let sysinfo_worker = SysInfoWorker::start();
    let app = gpui_platform::application().with_assets(app_ui::Assets);
    app.run(move |cx: &mut App| {
        set_config_app_id(MONITOR_CONFIG_APP_ID);
        init_tracing(MONITOR_CONFIG_APP_ID);
        app_ui::init_editor_shell(cx);

        let window_options = WindowOptions {
            titlebar: Some(app_ui::TitleBar::title_bar_options()),
            window_bounds: Some(WindowBounds::centered(size(px(1480.), px(980.)), cx)),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            let window_handle = cx
                .open_window(window_options, |window, cx| {
                    window.set_window_title("CyberMonitor");

                    let mode = cx.theme().mode;
                    app_ui::theme::apply_set("CyberMonitor", mode, cx);

                    window.on_window_should_close(cx, |window, _cx| {
                        tray::hide_window(window);
                        false
                    });

                    if start_hidden {
                        tray::hide_window(window);
                    } else {
                        window.activate_window();
                    }

                    let worker = sysinfo_worker.clone();
                    let view = cx.new(|cx| SysMonitorApp::new(window, cx, worker));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open CyberMonitor window");

            cx.spawn({
                async move |cx| loop {
                    Timer::after(Duration::from_millis(200)).await;
                    for command in tray::take_commands() {
                        match command {
                            TrayCommand::ShowWindow => {
                                let _ = window_handle.update(cx, |_, window, _| {
                                    tray::show_window(window);
                                    window.activate_window();
                                });
                            }
                            TrayCommand::ExitApp => {
                                let _ = window_handle.update(cx, |_, window, _| {
                                    window.remove_window();
                                });
                                cx.update(|cx| cx.quit());
                                return;
                            }
                        }
                    }
                }
            })
            .detach();
        })
        .detach();
    });
}
