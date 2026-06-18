//! Monitor connection settings for CyberMonitor.

use std::fs;
use std::path::PathBuf;

use gpui::{App, BorrowAppContext, Global, IntoElement, ParentElement, SharedString, Styled};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    ActiveTheme as _, Icon, IconName,
};
use serde::{Deserialize, Serialize};

use crate::monitor_sender::MonitorSenderHandle;

const DEFAULT_MONITOR_HOST: &str = "127.0.0.1";
const DEFAULT_MONITOR_PORT: u16 = 20379;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonitorConnectionConfig {
    pub host: String,
    pub port: u16,
    pub auto_connect: bool,
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

pub fn monitor_connection_config_path() -> PathBuf {
    let mut base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.push("CyberDesktop");
    base.push("CyberMonitor");
    base.push("connection.json");
    base
}

pub fn load_monitor_connection_config() -> MonitorConnectionConfig {
    let path = monitor_connection_config_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<MonitorConnectionConfig>(&content).ok())
        .unwrap_or_default()
}

pub fn save_monitor_connection_config(config: &MonitorConnectionConfig) {
    let path = monitor_connection_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}

/// Global state that lets the settings panel read/write the connection config
/// and control the telemetry sender without holding a reference to [`SysMonitorApp`].
pub struct MonitorConnectionGlobal {
    pub config: MonitorConnectionConfig,
    pub sender: Option<MonitorSenderHandle>,
}

impl Global for MonitorConnectionGlobal {}

pub fn init_monitor_connection(
    cx: &mut App,
    sender: MonitorSenderHandle,
    config: MonitorConnectionConfig,
) {
    if config.auto_connect {
        sender.connect(config.host.clone(), config.port);
    } else {
        sender.disconnect();
    }
    cx.set_global(MonitorConnectionGlobal {
        config,
        sender: Some(sender),
    });
}

fn with_config<T>(cx: &App, f: impl FnOnce(&MonitorConnectionConfig) -> T) -> T {
    f(&cx.global::<MonitorConnectionGlobal>().config)
}

fn update_config(cx: &mut App, f: impl FnOnce(&mut MonitorConnectionConfig)) {
    cx.update_global::<MonitorConnectionGlobal, _>(|g, _cx| {
        f(&mut g.config);
        save_monitor_connection_config(&g.config);
    });
}

pub fn build_monitor_settings(cx: &App) -> Settings {
    app_ui::build_editor_settings(cx).page(build_monitor_page(cx))
}

pub fn build_monitor_page(_cx: &App) -> SettingPage {
    let server_icon = Icon::new(IconName::File).path("icons/tabler/server.svg");

    SettingPage::new("推送到 Host")
        .icon(server_icon)
        .default_open(true)
        .groups(vec![SettingGroup::new().title("连接设置").items(vec![
            SettingItem::new(
                "Host",
                SettingField::input(
                    |cx: &App| with_config(cx, |c| c.host.clone().into()),
                    |value: SharedString, cx: &mut App| {
                        update_config(cx, |c| c.host = value.to_string());
                    },
                ),
            )
            .description("监控数据要推送到的目标主机地址"),
            SettingItem::new(
                "Port",
                SettingField::input(
                    |cx: &App| with_config(cx, |c| c.port.to_string().into()),
                    |value: SharedString, cx: &mut App| {
                        if let Ok(port) = value.parse::<u16>() {
                            update_config(cx, |c| c.port = port);
                        }
                    },
                ),
            )
            .description("目标主机的 WebSocket 端口"),
            SettingItem::new(
                "自动连接",
                SettingField::switch(
                    |cx: &App| with_config(cx, |c| c.auto_connect),
                    |enabled: bool, cx: &mut App| {
                        update_config(cx, |c| c.auto_connect = enabled);
                    },
                ),
            )
            .description("启动时自动连接到目标主机"),
            SettingItem::new(
                "操作",
                SettingField::element(
                    |_: &gpui_component::setting::RenderOptions,
                     _: &mut gpui::Window,
                     cx: &mut gpui::App|
                     -> gpui::AnyElement {
                        let connected = cx
                            .global::<MonitorConnectionGlobal>()
                            .sender
                            .as_ref()
                            .map(|s| s.status().connected)
                            .unwrap_or(false);

                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                Button::new("monitor-settings-connect")
                                    .primary()
                                    .label("连接 / 重连")
                                    .icon(Icon::new(IconName::File).path("icons/tabler/plug.svg"))
                                    .on_click(|_, _, cx| {
                                        cx.update_global::<MonitorConnectionGlobal, _>(|g, _cx| {
                                            if let Some(sender) = &g.sender {
                                                sender
                                                    .connect(g.config.host.clone(), g.config.port);
                                                g.config.auto_connect = true;
                                                save_monitor_connection_config(&g.config);
                                            }
                                        });
                                        cx.refresh_windows();
                                    }),
                            )
                            .child(
                                Button::new("monitor-settings-disconnect")
                                    .outline()
                                    .label("断开")
                                    .icon(
                                        Icon::new(IconName::File).path("icons/tabler/plug-off.svg"),
                                    )
                                    .on_click(|_, _, cx| {
                                        cx.update_global::<MonitorConnectionGlobal, _>(|g, _cx| {
                                            if let Some(sender) = &g.sender {
                                                sender.disconnect();
                                                g.config.auto_connect = false;
                                                save_monitor_connection_config(&g.config);
                                            }
                                        });
                                        cx.refresh_windows();
                                    }),
                            )
                            .child(
                                Label::new(if connected { "已连接" } else { "未连接" })
                                    .text_sm()
                                    .text_color(if connected {
                                        cx.theme().green
                                    } else {
                                        cx.theme().muted_foreground
                                    }),
                            )
                            .into_any_element()
                    },
                ),
            ),
        ])])
}
