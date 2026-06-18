use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;

const MIN_RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RECONNECT_INTERVAL: Duration = Duration::from_secs(30);
const RECONNECT_BACKOFF_MULTIPLIER: u32 = 2;

type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;
type WsSource = futures_util::stream::SplitStream<WsStream>;

#[derive(Clone, Default)]
struct SenderDesired {
    host: String,
    port: u16,
    enabled: bool,
    revision: u64,
}

#[derive(Clone, Default)]
pub struct SenderStatus {
    pub connected: bool,
    pub target: String,
    pub state: String,
    pub last_error: String,
}

#[derive(Clone)]
pub struct MonitorSenderHandle {
    desired: Arc<Mutex<SenderDesired>>,
    latest_payload: Arc<Mutex<Vec<u8>>>,
    status: Arc<Mutex<SenderStatus>>,
}

impl MonitorSenderHandle {
    pub fn new() -> Self {
        let desired = Arc::new(Mutex::new(SenderDesired {
            host: "127.0.0.1".to_string(),
            port: 20379,
            enabled: true,
            revision: 0,
        }));
        let latest_payload = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(SenderStatus {
            connected: false,
            target: "ws://127.0.0.1:20379/sys/info".to_string(),
            state: "Connecting".to_string(),
            last_error: String::new(),
        }));

        spawn_sender_thread(desired.clone(), latest_payload.clone(), status.clone());

        Self {
            desired,
            latest_payload,
            status,
        }
    }

    pub fn set_latest_payload(&self, payload: Vec<u8>) {
        if let Ok(mut latest) = self.latest_payload.lock() {
            *latest = payload;
        }
    }

    pub fn connect(&self, host: String, port: u16) {
        if let Ok(mut desired) = self.desired.lock() {
            desired.host = host;
            desired.port = port;
            desired.enabled = true;
            desired.revision = desired.revision.saturating_add(1);
        }
    }

    pub fn disconnect(&self) {
        if let Ok(mut desired) = self.desired.lock() {
            desired.enabled = false;
            desired.revision = desired.revision.saturating_add(1);
        }
    }

    pub fn status(&self) -> SenderStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }
}

fn spawn_sender_thread(
    desired: Arc<Mutex<SenderDesired>>,
    latest_payload: Arc<Mutex<Vec<u8>>>,
    status: Arc<Mutex<SenderStatus>>,
) {
    thread::spawn(move || {
        let runtime = Runtime::new().expect("failed to create monitor sender runtime");
        runtime.block_on(async move {
            let mut active_revision = 0_u64;
            let mut target = String::new();
            let mut sink = None::<WsSink>;
            let mut reconnect_interval = MIN_RECONNECT_INTERVAL;
            let reconnect_needed = Arc::new(AtomicBool::new(false));

            loop {
                let desired_snapshot = desired
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_default();
                let current_target = format!(
                    "ws://{}:{}/sys/info",
                    desired_snapshot.host, desired_snapshot.port
                );

                if !desired_snapshot.enabled {
                    sink = None;
                    reconnect_needed.store(false, Ordering::Relaxed);
                    active_revision = desired_snapshot.revision;
                    reconnect_interval = MIN_RECONNECT_INTERVAL;
                    update_status(&status, false, current_target, "Idle", "");
                    sleep(Duration::from_millis(300)).await;
                    continue;
                }

                let target_changed =
                    desired_snapshot.revision != active_revision || target != current_target;
                let should_reconnect = reconnect_needed.swap(false, Ordering::Relaxed)
                    || sink.is_none()
                    || target_changed;

                if should_reconnect {
                    sink = None;
                    target = current_target.clone();
                    active_revision = desired_snapshot.revision;
                    reconnect_needed.store(false, Ordering::Relaxed);
                    update_status(&status, false, current_target.clone(), "Connecting", "");
                    match connect_async(&current_target).await {
                        Ok((stream, _)) => {
                            reconnect_interval = MIN_RECONNECT_INTERVAL;
                            let (stream_sink, stream_source) = stream.split();
                            spawn_read_guard(stream_source, reconnect_needed.clone());
                            sink = Some(stream_sink);
                            update_status(&status, true, current_target, "Connected", "");
                        }
                        Err(err) => {
                            update_status(
                                &status,
                                false,
                                current_target,
                                "ConnectFailed",
                                &err.to_string(),
                            );
                            sleep(reconnect_interval).await;
                            reconnect_interval = (reconnect_interval
                                * RECONNECT_BACKOFF_MULTIPLIER)
                                .min(MAX_RECONNECT_INTERVAL);
                            continue;
                        }
                    }
                }

                let Some(stream_sink) = &mut sink else {
                    sleep(Duration::from_millis(300)).await;
                    continue;
                };

                let payload = latest_payload
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_default();
                if payload.is_empty() {
                    sleep(Duration::from_millis(300)).await;
                    continue;
                }

                if let Err(err) = stream_sink.send(Message::Binary(payload.into())).await {
                    update_status(
                        &status,
                        false,
                        target.clone(),
                        "SendFailed",
                        &err.to_string(),
                    );
                    sink = None;
                    reconnect_interval = MIN_RECONNECT_INTERVAL;
                    sleep(MIN_RECONNECT_INTERVAL).await;
                    continue;
                }

                update_status(&status, true, target.clone(), "Connected", "");
                sleep(Duration::from_secs(1)).await;
            }
        });
    });
}

fn spawn_read_guard(mut source: WsSource, reconnect_needed: Arc<AtomicBool>) {
    tokio::spawn(async move {
        loop {
            match source.next().await {
                Some(Ok(Message::Close(_))) | Some(Ok(Message::Frame(_))) | None => {
                    reconnect_needed.store(true, Ordering::Relaxed);
                    break;
                }
                Some(Err(_)) => {
                    reconnect_needed.store(true, Ordering::Relaxed);
                    break;
                }
                Some(Ok(_)) => continue,
            }
        }
    });
}

fn update_status(
    status: &Arc<Mutex<SenderStatus>>,
    connected: bool,
    target: String,
    state: &str,
    last_error: &str,
) {
    if let Ok(mut current) = status.lock() {
        current.connected = connected;
        current.target = target;
        current.state = state.to_string();
        current.last_error = last_error.to_string();
    }
}
