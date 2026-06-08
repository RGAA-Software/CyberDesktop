use std::sync::Arc;

use futures::future::BoxFuture;
use http::header::HeaderValue;
use http_client::{http, AsyncBody, HttpClient, Response, Url};

/// A simple HTTP client implementation using reqwest's blocking client.
/// This is used to enable image loading from URLs in gpui's asset system.
pub struct SimpleHttpClient {
    client: reqwest::blocking::Client,
}

impl SimpleHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl HttpClient for SimpleHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        req: http::Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let client = self.client.clone();
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let request = client
                .request(parts.method, &parts.uri.to_string())
                .headers(parts.headers);

            let request = request.body(match body.0 {
                http_client::Inner::Empty => reqwest::blocking::Body::from(""),
                http_client::Inner::Bytes(cursor) => {
                    reqwest::blocking::Body::from(cursor.into_inner().to_vec())
                }
                http_client::Inner::AsyncReader(_) => {
                    return Err(anyhow::anyhow!("AsyncReader body not supported"));
                }
            });

            let response = request.send()?;
            let status = response.status();
            let headers = response.headers().clone();
            let bytes = response.bytes()?;

            let body = AsyncBody::from_bytes(bytes);
            let mut builder = http::Response::builder().status(status.as_u16());
            *builder.headers_mut().unwrap() = headers;
            builder.body(body).map_err(|e: http::Error| anyhow::anyhow!(e))
        })
    }
}

/// Initialize the HTTP client for the application.
/// This must be called before any URL-based image assets are loaded.
pub fn init(cx: &mut gpui::App) {
    cx.set_http_client(Arc::new(SimpleHttpClient::new()));
}
