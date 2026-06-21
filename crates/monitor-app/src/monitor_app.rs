use std::time::Duration;

use gpui::{
    div, prelude::FluentBuilder, px, size, App, AppContext, Context, Entity, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
    WindowBounds, WindowOptions,
};
use gpui_component::{
    h_flex, input::InputState, label::Label, v_flex, ActiveTheme, Icon, IconName, Root, Sizable,
    StyledExt, ThemeMode, VirtualListScrollHandle,
};
use smol::Timer;

use files_core::{init_tracing, set_config_app_id, MONITOR_CONFIG_APP_ID};

use crate::monitor_actions::{
    CycleProcessSort, ProcessActionHandler, RestartServiceAction, ResumeProcess, RevealProcessExe,
    RevealStartupItem, SetProcessAffinity, SetProcessIoPriority, SetProcessPriority,
    ShowProcessDetails, StartServiceAction, StopServiceAction, SuspendProcess, TerminateProcess,
    TerminateProcessTree,
};
use crate::monitor_codec::encode_telemetry;
use crate::monitor_dashboard::render_dashboard;
use crate::monitor_model::{
    MachineTelemetry, MonitorTab, ProcessSort, ProcessSortColumn, SortDirection,
};
use crate::monitor_process_ctrl;
use crate::monitor_process_details::ProcessDetailsView;
use crate::monitor_sender::MonitorSenderHandle;
use crate::monitor_settings::{
    build_monitor_settings, init_monitor_connection, load_monitor_connection_config,
};
use crate::sys_info::SysProcessInfo;
use crate::sys_info_mgr::SysInfoManager;
use crate::tray::{self, TrayCommand};

const INTERVAL: Duration = Duration::from_secs(1);

pub struct SysMonitorApp {
    manager: SysInfoManager,
    telemetry: MachineTelemetry,
    active_tab: MonitorTab,
    sender: MonitorSenderHandle,
    process_scroll: VirtualListScrollHandle,
    process_search: Entity<InputState>,
    process_sort: ProcessSort,
    service_scroll: VirtualListScrollHandle,
    service_search: Entity<InputState>,
    startup_scroll: VirtualListScrollHandle,
    startup_search: Entity<InputState>,
    user_search: Entity<InputState>,
}

impl ProcessActionHandler for SysMonitorApp {
    fn terminate_process(&mut self, pid: u32, cx: &mut Context<Self>) {
        if self.manager.kill_process(pid) {
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
        let ok = self.manager.start_service(name);
        if ok {
            cx.notify();
        }
        ok
    }

    fn stop_service(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        let ok = self.manager.stop_service(name);
        if ok {
            cx.notify();
        }
        ok
    }

    fn restart_service(&mut self, name: &str, cx: &mut Context<Self>) -> bool {
        if self.manager.stop_service(name) {
            let ok = self.manager.start_service(name);
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
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut manager = SysInfoManager::new();
        let current = manager.load_system_info();
        let telemetry = MachineTelemetry::new(current.clone());
        let connection_config = load_monitor_connection_config();
        let sender = MonitorSenderHandle::new();
        sender.set_latest_payload(encode_telemetry(&current).unwrap_or_default());
        init_monitor_connection(cx, sender.clone(), connection_config);

        let process_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索进程..."));
        let service_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索服务..."));
        let startup_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索启动项..."));
        let user_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索用户..."));
        let this = Self {
            manager,
            telemetry,
            active_tab: MonitorTab::Overview,
            sender,
            process_scroll: VirtualListScrollHandle::new(),
            process_search,
            process_sort: ProcessSort::default(),
            service_scroll: VirtualListScrollHandle::new(),
            service_search,
            startup_scroll: VirtualListScrollHandle::new(),
            startup_search,
            user_search,
        };

        cx.spawn(async move |this, cx| loop {
            Timer::after(INTERVAL).await;
            if this
                .update(cx, |this, cx| {
                    this.refresh();
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
        .detach();

        this
    }

    fn refresh(&mut self) {
        let snapshot = self.manager.load_system_info();
        let payload = encode_telemetry(&snapshot).unwrap_or_default();
        self.telemetry.apply_snapshot(snapshot);
        self.sender.set_latest_payload(payload);
    }

    fn set_active_tab(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = MonitorTab::from_index(index);
        cx.notify();
    }
}

impl Render for SysMonitorApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab_index = self.active_tab as usize;
        let theme_icon = if cx.theme().mode.is_dark() {
            IconName::Moon
        } else {
            IconName::Sun
        };
        let view = cx.entity().clone();
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(Self::on_terminate_process))
            .on_action(cx.listener(Self::on_reveal_process_exe))
            .on_action(cx.listener(Self::on_show_process_details))
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
            .child(
                app_ui::TitleBar::new()
                    .h(px(35.))
                    .bg(cx.theme().title_bar)
                    .child(
                        h_flex()
                            .id("title-bar-inner")
                            .h_full()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .child(
                                h_flex()
                                    .id("app-logo")
                                    .flex_none()
                                    .items_center()
                                    .gap(px(8.))
                                    .pr(px(12.))
                                    .child(
                                        Label::new("CyberMonitor")
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                h_flex()
                                    .id("title-bar-actions")
                                    .flex_none()
                                    .items_center()
                                    .gap(px(6.))
                                    .px(px(10.))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(
                                        app_ui::toolbar_icon_button("monitor-theme-toggle")
                                            .icon(app_ui::toolbar_icon(theme_icon))
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
                                        app_ui::toolbar_icon_button("monitor-settings")
                                            .icon(app_ui::toolbar_icon(IconName::Settings2))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|_this, _e, _w, cx| {
                                                    cx.stop_propagation();
                                                    app_ui::SettingsWindowState::open_with(
                                                        cx,
                                                        |cx| build_monitor_settings(cx),
                                                        Some(px(35.)),
                                                    );
                                                }),
                                            ),
                                    )
                                    .child(
                                        app_ui::toolbar_icon_button("monitor-github")
                                            .icon(app_ui::toolbar_icon(IconName::Github))
                                            .on_click(|_, _, cx| {
                                                cx.open_url(app_ui::GITHUB_REPO_URL)
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .child(
                        v_flex()
                            .w(px(200.))
                            .h_full()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .p_2()
                            .gap_1()
                            .children(
                                [
                                    ("总览", IconName::ChartPie),
                                    ("CPU", IconName::Cpu),
                                    ("内存", IconName::MemoryStick),
                                    ("GPU", IconName::Frame),
                                    ("存储", IconName::HardDrive),
                                    ("网络", IconName::Network),
                                    ("进程", IconName::SquareTerminal),
                                    ("服务", IconName::Settings),
                                    ("启动项", IconName::Play),
                                    ("用户", IconName::User),
                                ]
                                .iter()
                                .enumerate()
                                .map(|(index, (label, icon))| {
                                    let active = active_tab_index == index;
                                    let text_color = if active {
                                        cx.theme().accent_foreground
                                    } else {
                                        cx.theme().foreground
                                    };
                                    div()
                                        .id(format!("sidebar-item-{index}"))
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .when(active, |this| this.bg(cx.theme().accent))
                                        .when(!active, |this| {
                                            this.hover(|style| {
                                                style.bg(cx.theme().muted.opacity(0.15))
                                            })
                                        })
                                        .child(
                                            h_flex()
                                                .gap_3()
                                                .items_center()
                                                .child(
                                                    Icon::new(icon.clone())
                                                        .small()
                                                        .text_color(text_color),
                                                )
                                                .child(
                                                    Label::new(label.to_string())
                                                        .text_sm()
                                                        .text_color(text_color),
                                                ),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.set_active_tab(index, window, cx);
                                        }))
                                }),
                            ),
                    )
                    .child(
                        div().flex_1().child(
                            v_flex()
                                .size_full()
                                .child(render_dashboard(
                                    &self.telemetry,
                                    self.active_tab,
                                    &self.process_scroll,
                                    &self.process_search,
                                    self.process_sort,
                                    &self.service_scroll,
                                    &self.service_search,
                                    &self.startup_scroll,
                                    &self.startup_search,
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
                                ))
                                .child(div().h(px(15.))),
                        ),
                    ),
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
                .manager
                .get_process_details(action.pid)
                .unwrap_or_default();
            ProcessDetailsView::open(process.clone(), details, cx);
        }
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

                    let view = cx.new(|cx| SysMonitorApp::new(window, cx));
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
