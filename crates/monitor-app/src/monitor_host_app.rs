use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use clap::Parser;
use futures_util::StreamExt;
use gpui::{
    div, prelude::FluentBuilder as _, px, size, App, AppContext, Context, Entity,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, Styled, Window,
    WindowBounds, WindowOptions,
};
use gpui_component::{
    h_flex, input::InputState, label::Label, list::ListItem, scroll::ScrollableElement, v_flex,
    ActiveTheme, Icon, IconName, Root, StyledExt, ThemeMode, VirtualListScrollHandle,
};
use smol::Timer;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

use files_core::{init_tracing, set_config_app_id, MONITOR_CONFIG_APP_ID};

use crate::monitor_actions::{
    CycleProcessSort, ProcessActionHandler, RevealProcessExe, RevealStartupItem,
    ShowProcessDetails, TerminateProcess,
};
use crate::monitor_dashboard::{render_connection_summary, render_dashboard};
use crate::monitor_model::{
    MachineTelemetry, MonitorTab, ProcessSort, ProcessSortColumn, RemoteMachineState, SortDirection,
};
use crate::monitor_process_details::ProcessDetailsView;
use crate::sys_info::SysInfo;
use crate::tray::{self, TrayCommand};

const PATH_SYS_INFO: &str = "/sys/info";
const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Parser, Clone)]
#[command(name = "CyberMonitorHost", version, about)]
pub struct HostCli {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, default_value_t = 20379)]
    port: u16,

    #[arg(long, default_value_t = false)]
    pub startup: bool,
}

#[derive(Clone, Default)]
struct HostServerStatus {
    listen_addr: String,
    state: String,
    last_error: String,
}

#[derive(Clone)]
struct HostServerHandle {
    machines: Arc<Mutex<BTreeMap<String, RemoteMachineState>>>,
    status: Arc<Mutex<HostServerStatus>>,
}

impl HostServerHandle {
    fn new(listen_host: String, listen_port: u16) -> Self {
        let machines = Arc::new(Mutex::new(BTreeMap::new()));
        let status = Arc::new(Mutex::new(HostServerStatus {
            listen_addr: format!("{listen_host}:{listen_port}"),
            state: "Starting".to_string(),
            last_error: String::new(),
        }));

        spawn_host_server(listen_host, listen_port, machines.clone(), status.clone());

        Self { machines, status }
    }

    fn machines(&self) -> Vec<RemoteMachineState> {
        self.machines
            .lock()
            .map(|items| items.values().cloned().collect())
            .unwrap_or_default()
    }

    fn status(&self) -> HostServerStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }
}

fn spawn_host_server(
    listen_host: String,
    listen_port: u16,
    machines: Arc<Mutex<BTreeMap<String, RemoteMachineState>>>,
    status: Arc<Mutex<HostServerStatus>>,
) {
    thread::spawn(move || {
        let runtime = Runtime::new().expect("failed to create CyberMonitorHost runtime");
        runtime.block_on(async move {
            let listen_addr = format!("{listen_host}:{listen_port}");
            match TcpListener::bind(&listen_addr).await {
                Ok(listener) => {
                    update_host_status(&status, &listen_addr, "Listening", "");
                    loop {
                        match listener.accept().await {
                            Ok((stream, _)) => {
                                let machines = machines.clone();
                                let status = status.clone();
                                tokio::spawn(async move {
                                    let _ = handle_host_client(stream, machines, status).await;
                                });
                            }
                            Err(err) => {
                                update_host_status(
                                    &status,
                                    &listen_addr,
                                    "AcceptFailed",
                                    &err.to_string(),
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    update_host_status(&status, &listen_addr, "BindFailed", &err.to_string());
                }
            }
        });
    });
}

async fn handle_host_client(
    stream: TcpStream,
    machines: Arc<Mutex<BTreeMap<String, RemoteMachineState>>>,
    status: Arc<Mutex<HostServerStatus>>,
) -> Result<(), String> {
    let peer_addr = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    let peer_ip = stream
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    let ws_stream = accept_hdr_async(stream, move |req: &Request, resp: Response| {
        if req.uri().path() == PATH_SYS_INFO {
            Ok(resp)
        } else {
            Err(
                tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(Some(
                    "invalid path".to_string(),
                )),
            )
        }
    })
    .await
    .map_err(|err| err.to_string())?;

    let (_sink, mut source) = ws_stream.split();
    let mut machine_key = None::<String>;

    while let Some(message) = source.next().await {
        let message = message.map_err(|err| err.to_string())?;
        let Message::Text(text) = message else {
            continue;
        };

        match serde_json::from_str::<SysInfo>(&text) {
            Ok(info) => {
                let host_name = if info.os.sys_host_name.is_empty() {
                    "UnknownHost".to_string()
                } else {
                    info.os.sys_host_name.clone()
                };
                let key = peer_ip.clone();
                machine_key = Some(key.clone());

                if let Ok(mut items) = machines.lock() {
                    match items.get_mut(&key) {
                        Some(record) => {
                            record.display_name = host_name.clone();
                            record.peer_addr = peer_addr.clone();
                            record.peer_ip = peer_ip.clone();
                            record.active_peer_addr = peer_addr.clone();
                            record.last_seen = info.timestamp_readable.clone();
                            record.connected = true;
                            record.telemetry.apply_snapshot(info);
                        }
                        None => {
                            items.insert(
                                key.clone(),
                                RemoteMachineState {
                                    machine_id: key.clone(),
                                    display_name: host_name,
                                    peer_ip: peer_ip.clone(),
                                    peer_addr: peer_addr.clone(),
                                    active_peer_addr: peer_addr.clone(),
                                    last_seen: info.timestamp_readable.clone(),
                                    connected: true,
                                    telemetry: MachineTelemetry::new(info),
                                },
                            );
                        }
                    }
                }
            }
            Err(err) => {
                let snapshot = status.lock().map(|item| item.clone()).unwrap_or_default();
                update_host_status(
                    &status,
                    &snapshot.listen_addr,
                    "ParseFailed",
                    &err.to_string(),
                );
            }
        }
    }

    if let Some(machine_key) = machine_key {
        if let Ok(mut items) = machines.lock() {
            if let Some(record) = items.get_mut(&machine_key) {
                if record.active_peer_addr == peer_addr {
                    record.connected = false;
                }
            }
        }
    }

    Ok(())
}

fn update_host_status(
    status: &Arc<Mutex<HostServerStatus>>,
    listen_addr: &str,
    state: &str,
    last_error: &str,
) {
    if let Ok(mut current) = status.lock() {
        current.listen_addr = listen_addr.to_string();
        current.state = state.to_string();
        current.last_error = last_error.to_string();
    }
}

pub struct SysMonitorHostApp {
    server: HostServerHandle,
    machines: Vec<RemoteMachineState>,
    selected_machine: Option<String>,
    server_status: HostServerStatus,
    active_tab: MonitorTab,
    process_scroll: VirtualListScrollHandle,
    process_search: Entity<InputState>,
    process_sort: ProcessSort,
    service_search: Entity<InputState>,
    startup_scroll: VirtualListScrollHandle,
    startup_search: Entity<InputState>,
    user_search: Entity<InputState>,
}

impl ProcessActionHandler for SysMonitorHostApp {}

impl SysMonitorHostApp {
    fn new(_window: &mut Window, cx: &mut Context<Self>, cli: HostCli) -> Self {
        let server = HostServerHandle::new(cli.host, cli.port);
        let process_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索进程..."));
        let service_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索服务..."));
        let startup_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索启动项..."));
        let user_search = cx.new(|cx| InputState::new(_window, cx).placeholder("搜索用户..."));
        let mut this = Self {
            server,
            machines: Vec::new(),
            selected_machine: None,
            server_status: HostServerStatus::default(),
            active_tab: MonitorTab::Overview,
            process_scroll: VirtualListScrollHandle::new(),
            process_search,
            process_sort: ProcessSort::default(),
            service_search,
            startup_scroll: VirtualListScrollHandle::new(),
            startup_search,
            user_search,
        };
        this.refresh();

        cx.spawn(async move |this, cx| loop {
            Timer::after(POLL_INTERVAL).await;
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
        self.server_status = self.server.status();
        self.machines = self.server.machines();
        self.machines
            .sort_by(|a, b| a.display_name.cmp(&b.display_name));

        let selected_exists = self.selected_machine.as_ref().is_some_and(|selected| {
            self.machines
                .iter()
                .any(|item| &item.machine_id == selected)
        });
        if !selected_exists {
            self.selected_machine = self.machines.first().map(|item| item.machine_id.clone());
        }
    }

    fn selected_machine(&self) -> Option<&RemoteMachineState> {
        self.selected_machine.as_ref().and_then(|selected| {
            self.machines
                .iter()
                .find(|item| &item.machine_id == selected)
        })
    }

    fn select_machine(
        &mut self,
        machine_id: &String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_machine = Some(machine_id.clone());
        cx.notify();
    }

    fn set_active_tab(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = MonitorTab::from_index(index);
        cx.notify();
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(280.))
            .h_full()
            .gap_2()
            .p_3()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                Label::new("在线机器")
                    .text_sm()
                    .text_color(cx.theme().foreground),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .gap_2()
                            .children(self.machines.iter().map(|machine| {
                                let selected = self
                                    .selected_machine
                                    .as_ref()
                                    .is_some_and(|item| item == &machine.machine_id);
                                let machine_id = machine.machine_id.clone();
                                ListItem::new(format!("machine-{}", machine.machine_id))
                                    .selected(selected)
                                    .w_full()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .h(px(50.))
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                v_flex()
                                                    .justify_center()
                                                    .items_start()
                                                    .gap(px(3.))
                                                    .child(
                                                        Label::new(machine.peer_ip.clone())
                                                            .text_sm()
                                                            .font_semibold()
                                                            .text_color(cx.theme().foreground),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .items_center()
                                                            .child(
                                                                Label::new(if machine.connected {
                                                                    "在线".to_string()
                                                                } else {
                                                                    "离线".to_string()
                                                                })
                                                                .text_xs()
                                                                .text_color(if machine.connected {
                                                                    cx.theme().green
                                                                } else {
                                                                    cx.theme().red
                                                                }),
                                                            )
                                                            .child(
                                                                Label::new(
                                                                    machine.display_name.clone(),
                                                                )
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                ),
                                                            ),
                                                    ),
                                            )
                                            .when(selected, |this| {
                                                this.child(
                                                    div().pr(px(10.)).child(
                                                        Icon::new(IconName::Check)
                                                            .text_color(cx.theme().green),
                                                    ),
                                                )
                                            }),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select_machine(&machine_id, window, cx);
                                    }))
                            })),
                    ),
            )
    }

    fn render_empty(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex().size_full().items_center().justify_center().child(
            Label::new("当前没有机器连接到 /sys/info")
                .text_sm()
                .text_color(cx.theme().muted_foreground),
        )
    }
}

impl Render for SysMonitorHostApp {
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
            .on_action(cx.listener(Self::on_terminate_process))
            .on_action(cx.listener(Self::on_reveal_process_exe))
            .on_action(cx.listener(Self::on_show_process_details))
            .on_action(cx.listener(Self::on_reveal_startup_item))
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
                                        Label::new("CyberMonitorHost")
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
                                        app_ui::toolbar_icon_button("host-theme-toggle")
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
                                        app_ui::toolbar_icon_button("host-settings")
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
                                        app_ui::toolbar_icon_button("host-github")
                                            .icon(app_ui::toolbar_icon(IconName::Github))
                                            .on_click(|_, _, cx| {
                                                cx.open_url(app_ui::GITHUB_REPO_URL)
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(render_connection_summary(
                &format!("WebSocket 路径固定为 {}", PATH_SYS_INFO),
                cx,
            ))
            .child(
                h_flex()
                    .flex_1()
                    .items_start()
                    .child(self.render_sidebar(cx))
                    .child(
                        div().flex_1().h_full().min_h_0().overflow_hidden().child(
                            v_flex()
                                .size_full()
                                .items_start()
                                .when(self.selected_machine().is_none(), |this| {
                                    this.child(self.render_empty(cx))
                                })
                                .when_some(self.selected_machine(), |this, machine| {
                                    this.child(
                                        Label::new(format!(
                                            "{} | {} | 最后上报 {}",
                                            machine.display_name,
                                            machine.peer_addr,
                                            machine.last_seen
                                        ))
                                        .px_4()
                                        .py_3()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        app_ui::TabBar::new("host-monitor-tabs")
                                            .segmented()
                                            .px_3()
                                            .py_2()
                                            .selected_index(active_tab_index)
                                            .on_click(cx.listener(
                                                |this, ix: &usize, window, cx| {
                                                    this.set_active_tab(*ix, window, cx);
                                                },
                                            ))
                                            .child(app_ui::Tab::new().label("总览"))
                                            .child(app_ui::Tab::new().label("CPU / 内存"))
                                            .child(app_ui::Tab::new().label("GPU"))
                                            .child(app_ui::Tab::new().label("存储"))
                                            .child(app_ui::Tab::new().label("网络"))
                                            .child(app_ui::Tab::new().label("传感器"))
                                            .child(app_ui::Tab::new().label("进程"))
                                            .child(app_ui::Tab::new().label("服务"))
                                            .child(app_ui::Tab::new().label("启动项"))
                                            .child(app_ui::Tab::new().label("用户")),
                                    )
                                    .child({
                                        let view = cx.entity().clone();
                                        div().flex_1().w_full().min_h_0().overflow_hidden().child(
                                            div().size_full().overflow_y_scrollbar().child(
                                                render_dashboard(
                                                    &machine.telemetry,
                                                    self.active_tab,
                                                    &self.process_scroll,
                                                    &self.process_search,
                                                    self.process_sort,
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
                                                ),
                                            ),
                                        )
                                    })
                                }),
                        ),
                    ),
            )
    }
}

impl SysMonitorHostApp {
    fn on_terminate_process(
        &mut self,
        _action: &TerminateProcess,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn on_reveal_process_exe(
        &mut self,
        _action: &RevealProcessExe,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn on_show_process_details(
        &mut self,
        action: &ShowProcessDetails,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(machine) = self.selected_machine() {
            if let Some(process) = machine
                .telemetry
                .current
                .processes
                .iter()
                .find(|p| p.pid == action.pid)
            {
                ProcessDetailsView::open(
                    process.clone(),
                    crate::monitor_process_detail::ProcessDetailInfo::default(),
                    cx,
                );
            }
        }
    }

    fn on_reveal_startup_item(
        &mut self,
        _action: &RevealStartupItem,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
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
}

pub fn run(cli: HostCli) {
    tray::init_tray("CyberMonitorHost");
    let app = gpui_platform::application().with_assets(app_ui::Assets);
    app.run(move |cx: &mut App| {
        set_config_app_id(MONITOR_CONFIG_APP_ID);
        init_tracing(MONITOR_CONFIG_APP_ID);
        app_ui::init_editor_shell(cx);

        let window_options = WindowOptions {
            titlebar: Some(app_ui::TitleBar::title_bar_options()),
            window_bounds: Some(WindowBounds::centered(size(px(1600.), px(980.)), cx)),
            ..Default::default()
        };

        let cli = cli.clone();
        cx.spawn(async move |cx| {
            let start_hidden = cli.startup;
            let window_handle = cx
                .open_window(window_options, |window, cx| {
                    window.set_window_title("CyberMonitorHost");

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

                    let host_cli = cli.clone();
                    let view = cx.new(|cx| SysMonitorHostApp::new(window, cx, host_cli));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open CyberMonitorHost window");

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
