//! Previews: forward one HTTP request from the relay to a port on this computer's
//! localhost. Nothing here listens; the request arrived over the agent's own connection.

use bytes::Bytes;
use croncave_proto::{HttpHead, HttpRequestHead};
use futures::StreamExt;

use crate::stream::AgentStream;

const HOP_BY_HOP: &[&str] =
    &["connection", "keep-alive", "transfer-encoding", "upgrade", "proxy-connection", "te", "trailer"];

pub async fn handle(head: HttpRequestHead, s: &mut AgentStream) {
    let body = match s.read_body(32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            s.send_error(e).await;
            return;
        }
    };
    let client = match reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build() {
        Ok(c) => c,
        Err(e) => {
            s.send_error(e.to_string()).await;
            return;
        }
    };
    let Ok(method) = reqwest::Method::from_bytes(head.method.as_bytes()) else {
        s.send_error("unsupported method").await;
        return;
    };
    let path = if head.path.starts_with('/') { head.path.clone() } else { format!("/{}", head.path) };
    let mut req = client.request(method, format!("http://127.0.0.1:{}{}", head.port, path));
    for (k, v) in &head.headers {
        let lk = k.to_ascii_lowercase();
        if lk == "host" || HOP_BY_HOP.contains(&lk.as_str()) {
            continue;
        }
        req = req.header(k, v);
    }
    // Dev servers that check the host header accept localhost.
    req = req.header("host", format!("localhost:{}", head.port));
    let resp = match req.body(body).send().await {
        Ok(r) => r,
        Err(_) => {
            s.send_json(&HttpHead {
                status: 502,
                headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
            })
            .await;
            s.send(Bytes::from(format!("Nothing is answering on port {} of your computer yet.", head.port))).await;
            return;
        }
    };
    let headers = resp
        .headers()
        .iter()
        .filter(|(k, _)| !HOP_BY_HOP.contains(&k.as_str()))
        .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
        .collect();
    s.send_json(&HttpHead { status: resp.status().as_u16(), headers }).await;
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(c) => {
                if !s.send(c).await {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
