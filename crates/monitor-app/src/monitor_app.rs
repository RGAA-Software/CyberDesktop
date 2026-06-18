use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    div, px, size, App, AppContext, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, Styled, Window, WindowBounds, WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    v_flex, ActiveTheme, IconName, Root, StyledExt, ThemeMode,
};
use serde::{Deserialize, Serialize};
use smol::Timer;

use files_core::{init_tracing, set_config_app_id, MONITOR_CONFIG_APP_ID};

use crate::monitor_dashboard::{render_connection_summary, render_dashboard};
use crate::monitor_model::{MachineTelemetry, MonitorTab};
use crate::monitor_sender::{MonitorSenderHandle, SenderStatus};
use crate::sys_info_mgr::SysInfoManager;
use crate::tray::{self, TrayCommand};

const INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_MONITOR_HOST: &str = "127.0.0.1";
const DEFAULT_MONITOR_PORT: u16 = 20379;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MonitorConnectionConfig {
    host: String,
    port: u16,
    auto_connect: bool,
}

impl Default for MonitorConnectionConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_MONITOR_HOST.to_string(),
            port: DEFAULT_MONITOR_PORT,
            auto_connect: true,
        }
    }
}

fn monitor_connection_config_path() -> PathBuf {
    let mut base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.push("CyberDesktop");
    base.push("CyberMonitor");
    base.push("connection.json");
    base
}

fn load_monitor_connection_config() -> MonitorConnectionConfig {
    let path = monitor_connection_config_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<MonitorConnectionConfig>(&content).ok())
        .unwrap_or_default()
}

fn save_monitor_connection_config(config: &MonitorConnectionConfig) {
    let path = monitor_connection_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}

pub struct SysMonitorApp {
    manager: SysInfoManager,
    telemetry: MachineTelemetry,
    active_tab: MonitorTab,
    host_input: gpui::Entity<InputState>,
    port_input: gpui::Entity<InputState>,
    sender: MonitorSenderHandle,
    sender_status: SenderStatus,
}

impl SysMonitorApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut manager = SysInfoManager::new();
        let current = manager.load_system_info();
        let telemetry = MachineTelemetry::new(current.clone());
        let connection_config = load_monitor_connection_config();
        let sender = MonitorSenderHandle::new();
        sender.set_latest_json(serde_json::to_string(&current).unwrap_or_default());
        if connection_config.auto_connect {
            sender.connect(connection_config.host.clone(), connection_config.port);
        } else {
            sender.disconnect();
        }

        let host_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(&connection_config.host)
                .placeholder("host")
        });
        let port_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(connection_config.port.to_string())
                .placeholder("port")
        });

        let mut this = Self {
            manager,
            telemetry,
            active_tab: MonitorTab::Overview,
            host_input,
            port_input,
            sender,
            sender_status: SenderStatus::default(),
        };
        this.sender_status = this.sender.status();

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
        let json = serde_json::to_string(&snapshot).unwrap_or_default();
        self.telemetry.apply_snapshot(snapshot);
        self.sender.set_latest_json(json);
        self.sender_status = self.sender.status();
    }

    fn current_connection_config(
        &self,
        cx: &Context<Self>,
        auto_connect: bool,
    ) -> MonitorConnectionConfig {
        MonitorConnectionConfig {
            host: self.host_input.read(cx).value().to_string(),
            port: self
                .port_input
                .read(cx)
                .value()
                .parse::<u16>()
                .unwrap_or(DEFAULT_MONITOR_PORT),
            auto_connect,
        }
    }

    fn set_active_tab(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = MonitorTab::from_index(index);
        cx.notify();
    }

    fn connect_sender(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config = self.current_connection_config(cx, true);
        self.sender.connect(config.host.clone(), config.port);
        save_monitor_connection_config(&config);
        self.sender_status = self.sender.status();
        cx.notify();
    }

    fn disconnect_sender(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sender.disconnect();
        save_monitor_connection_config(&self.current_connection_config(cx, false));
        self.sender_status = self.sender.status();
        cx.notify();
    }

    fn render_connection_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .px_4()
            .py_3()
            .bg(cx.theme().secondary)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Label::new("推送到 Host")
                            .text_sm()
                            .text_color(cx.theme().foreground),
                    )
                    .child(div().w(px(240.)).child(Input::new(&self.host_input)))
                    .child(div().w(px(120.)).child(Input::new(&self.port_input)))
                    .child(
                        Button::new("monitor-connect")
                            .primary()
                            .label("Connect / Reconnect")
                            .on_click(cx.listener(Self::connect_sender)),
                    )
                    .child(
                        Button::new("monitor-disconnect")
                            .outline()
                            .label("Disconnect")
                            .on_click(cx.listener(Self::disconnect_sender)),
                    ),
            )
            .child(render_connection_summary(
                &format!(
                    "状态: {} | 目标: {}{}",
                    self.sender_status.state,
                    self.sender_status.target,
                    if self.sender_status.last_error.is_empty() {
                        String::new()
                    } else {
                        format!(" | 错误: {}", self.sender_status.last_error)
                    }
                ),
                cx,
            ))
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
        v_flex()
            .size_full()
            .bg(cx.theme().background)
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
                                                    app_ui::SettingsWindowState::open_editor(cx);
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
            .child(self.render_connection_panel(cx))
            .child(
                app_ui::TabBar::new("monitor-tabs")
                    .segmented()
                    .px_3()
                    .py_2()
                    .selected_index(active_tab_index)
                    .on_click(cx.listener(|this, ix: &usize, window, cx| {
                        this.set_active_tab(*ix, window, cx);
                    }))
                    .child(app_ui::Tab::new().label("总览"))
                    .child(app_ui::Tab::new().label("CPU / 内存"))
                    .child(app_ui::Tab::new().label("GPU"))
                    .child(app_ui::Tab::new().label("存储"))
                    .child(app_ui::Tab::new().label("网络"))
                    .child(app_ui::Tab::new().label("传感器")),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(render_dashboard(&self.telemetry, self.active_tab, cx)),
            )
    }
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
