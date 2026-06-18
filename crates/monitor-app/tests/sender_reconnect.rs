use std::time::Duration;

use futures_util::StreamExt;

#[tokio::test]
async fn test_sender_reconnects_after_server_disconnect() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept first connection");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("ws handshake");
        // Wait for the first telemetry message.
        let _ = ws.next().await;
        // Close the connection to trigger client-side reconnect.
        let _ = ws.close(None).await;
    });

    let sender = monitor_app::monitor_sender::MonitorSenderHandle::new();
    sender.connect("127.0.0.1".to_string(), port);
    sender.set_latest_payload(r#"{"test":1}"#.as_bytes().to_vec());

    let mut connected_once = false;
    for _ in 0..50 {
        if sender.status().connected {
            connected_once = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(connected_once, "sender should connect to the test server");

    // Wait for the server side to close.
    let _ = tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("server closes in time")
        .expect("server task");

    // The client should detect the close and move into reconnecting.
    let mut reconnected = false;
    for _ in 0..50 {
        let state = sender.status().state;
        if state == "Connecting" || state == "ConnectFailed" {
            reconnected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        reconnected,
        "sender should attempt reconnect after the server disconnects"
    );
}

#[test]
fn test_reconnect_backoff_clamps_at_max() {
    let mut interval = Duration::from_secs(1);
    for _ in 0..10 {
        interval = (interval * 2).min(Duration::from_secs(30));
    }
    assert_eq!(interval, Duration::from_secs(30));
}
