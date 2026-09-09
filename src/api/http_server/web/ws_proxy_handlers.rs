//! WebSocket proxies for KubeVirt serial console and VNC (inside http_server::web).

use super::*;
use axum::extract::{
    ws::{Message, WebSocket},
    Query, WebSocketUpgrade,
};
use futures_util::{SinkExt, StreamExt};
use secrecy::ExposeSecret;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame as TCloseFrame, Message as TMsg};

/// Translate a tungstenite close frame (from the KubeVirt side) into an axum one.
fn to_client_close(
    frame: Option<TCloseFrame<'_>>,
) -> Option<axum::extract::ws::CloseFrame<'static>> {
    frame.map(|f| axum::extract::ws::CloseFrame {
        code: f.code.into(),
        reason: f.reason.into_owned().into(),
    })
}

/// Translate an axum close frame (from the browser side) into a tungstenite one.
fn to_kube_close(frame: Option<axum::extract::ws::CloseFrame<'_>>) -> Option<TCloseFrame<'static>> {
    frame.map(|f| TCloseFrame {
        code: f.code.into(),
        reason: f.reason.into_owned().into(),
    })
}

#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    pub token: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
}

fn authorize(state: &WebState, token: Option<&str>) -> bool {
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return false;
    };
    state.auth.api_key_ok(token) || state.auth.validate_bearer(token).is_some()
}

/// TLS connector that trusts the Kubernetes API server CA from kubeconfig / in-cluster config.
fn kube_tls_connector(config: &kube::Config) -> anyhow::Result<tokio_tungstenite::Connector> {
    let mut builder = native_tls::TlsConnector::builder();
    if let Some(certs) = &config.root_cert {
        for der in certs {
            match native_tls::Certificate::from_der(der) {
                Ok(cert) => {
                    builder.add_root_certificate(cert);
                }
                Err(e) => log::warn!("skipping kube CA cert: {e}"),
            }
        }
    }
    // In-cluster cluster_url is often an IP (e.g. 10.43.0.1); the API CA CN/SAN
    // may not match. Trust the kube CA above, but skip hostname checks for WS.
    builder.danger_accept_invalid_hostnames(true);
    if config.accept_invalid_certs {
        builder.danger_accept_invalid_certs(true);
    }
    Ok(tokio_tungstenite::Connector::NativeTls(builder.build()?))
}

async fn build_kubevirt_ws(
    namespace: &str,
    name: &str,
    subresource: &str,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let config = kube::Config::infer().await?;
    let cluster = config.cluster_url.to_string();
    let cluster = cluster.trim_end_matches('/');
    let (ws_scheme, host) = if let Some(rest) = cluster.strip_prefix("https://") {
        ("wss", rest)
    } else if let Some(rest) = cluster.strip_prefix("http://") {
        ("ws", rest)
    } else {
        (
            "wss",
            cluster
                .trim_start_matches("wss://")
                .trim_start_matches("ws://"),
        )
    };
    let path = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachineinstances/{name}/{subresource}"
    );
    let url = format!("{ws_scheme}://{host}{path}");

    let proto = if subresource == "vnc" {
        "binary.kubevirt.io"
    } else {
        "plain.kubevirt.io"
    };

    let mut req =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)?;
    req.headers_mut().insert(
        http::header::SEC_WEBSOCKET_PROTOCOL,
        http::HeaderValue::from_static(proto),
    );

    // In-cluster kube config stores the SA path in token_file; kubeconfig may use token.
    let bearer = if let Some(token) = config.auth_info.token.as_ref() {
        Some(token.expose_secret().to_string())
    } else if let Some(path) = config.auth_info.token_file.as_ref() {
        Some(std::fs::read_to_string(path)?.trim().to_string())
    } else {
        None
    };
    if let Some(token) = bearer.filter(|t| !t.is_empty()) {
        req.headers_mut().insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_str(&format!("Bearer {token}"))?,
        );
    }

    let connector = kube_tls_connector(&config)?;
    let (ws, _) = tokio_tungstenite::connect_async_tls_with_config(
        req,
        None,
        false,
        Some(connector),
    )
    .await?;
    Ok(ws)
}

async fn proxy_kube_ws(
    client_ws: WebSocket,
    namespace: String,
    name: String,
    subresource: &'static str,
) {
    let mut client_ws = client_ws;
    let kube_ws = match build_kubevirt_ws(&namespace, &name, subresource).await {
        Ok(ws) => ws,
        Err(e) => {
            let _ = client_ws
                .send(Message::Text(format!(
                    "error: cannot open {subresource} for '{name}' (is the VMI running?): {e}"
                )))
                .await;
            let _ = client_ws.close().await;
            return;
        }
    };

    let (mut kube_sink, mut kube_stream) = kube_ws.split();
    let (mut client_sink, mut client_stream) = client_ws.split();

    let to_kube = async {
        while let Some(Ok(msg)) = client_stream.next().await {
            let mapped = match msg {
                Message::Text(t) => TMsg::Text(t),
                Message::Binary(b) => TMsg::Binary(b),
                Message::Ping(p) => TMsg::Ping(p),
                Message::Pong(p) => TMsg::Pong(p),
                Message::Close(frame) => {
                    let _ = kube_sink.send(TMsg::Close(to_kube_close(frame))).await;
                    break;
                }
            };
            if kube_sink.send(mapped).await.is_err() {
                break;
            }
        }
    };

    let to_client = async {
        while let Some(Ok(msg)) = kube_stream.next().await {
            let mapped = match msg {
                TMsg::Text(t) => Message::Text(t),
                TMsg::Binary(b) => Message::Binary(b),
                TMsg::Ping(p) => Message::Ping(p),
                TMsg::Pong(p) => Message::Pong(p),
                TMsg::Close(frame) => {
                    let _ = client_sink.send(Message::Close(to_client_close(frame))).await;
                    let _ = client_sink.close().await;
                    break;
                }
                TMsg::Frame(_) => break,
            };
            if client_sink.send(mapped).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = to_kube => {}
        _ = to_client => {}
    }
}

pub async fn ws_console(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(q): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    if !authorize(&s, q.token.as_deref()) {
        let (st, j) = err_json(401, "UNAUTHORIZED", "Missing or invalid token");
        return (st, j).into_response();
    }
    let namespace = s.namespace.clone();
    drop(s);
    ws.protocols(["plain.kubevirt.io"])
        .on_upgrade(move |socket| proxy_kube_ws(socket, namespace, name, "console"))
        .into_response()
}

pub async fn ws_vnc(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(q): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    if !authorize(&s, q.token.as_deref()) {
        let (st, j) = err_json(401, "UNAUTHORIZED", "Missing or invalid token");
        return (st, j).into_response();
    }
    let namespace = s.namespace.clone();
    drop(s);
    ws.protocols(["binary.kubevirt.io"])
        .on_upgrade(move |socket| proxy_kube_ws(socket, namespace, name, "vnc"))
        .into_response()
}

pub async fn ws_ssh(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    Path(name): Path<String>,
    Query(q): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    if !authorize(&s, q.token.as_deref()) {
        let (st, j) = err_json(401, "UNAUTHORIZED", "Missing or invalid token");
        return (st, j).into_response();
    }
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    let user = q.user.clone().unwrap_or_else(|| "zorvia".into());
    let port = q.port.unwrap_or(22);
    if let Err(e) = crate::kube::ssh::validate_ssh_user(&user) {
        let (st, j) = err_json(400, "INVALID", &e.to_string());
        return (st, j).into_response();
    }
    ws.on_upgrade(move |socket| proxy_ssh(socket, client, namespace, name, user, port))
        .into_response()
}

async fn proxy_ssh(
    mut client_ws: WebSocket,
    kube: crate::kube::KubeClient,
    namespace: String,
    name: String,
    user: String,
    port: u16,
) {
    let argv = match kube.get_vm_ip(&namespace, &name).await {
        Ok(Some(ip)) => match crate::kube::ssh::ssh_argv(&user, &ip, port) {
            Ok(v) => v,
            Err(e) => {
                let _ = client_ws
                    .send(Message::Text(format!("error: {e}")))
                    .await;
                let _ = client_ws.close().await;
                return;
            }
        },
        _ => match crate::kube::ssh::virtctl_ssh_argv(&user, &name, &namespace) {
            Ok(v) => v,
            Err(e) => {
                let _ = client_ws
                    .send(Message::Text(format!("error: {e}")))
                    .await;
                let _ = client_ws.close().await;
                return;
            }
        },
    };

    let _ = client_ws
        .send(Message::Text(format!(
            "Connecting via {} …\r\n",
            argv.first().cloned().unwrap_or_else(|| "ssh".into())
        )))
        .await;

    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("TERM", "xterm-256color");

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = client_ws
                .send(Message::Text(format!(
                    "error: failed to spawn {}: {e} (install openssh-client or virtctl)\r\n",
                    argv[0]
                )))
                .await;
            let _ = client_ws.close().await;
            return;
        }
    };

    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let (sink, mut stream) = client_ws.split();
    let sink = std::sync::Arc::new(tokio::sync::Mutex::new(sink));
    let sink_out = sink.clone();
    let sink_err = sink.clone();

    let to_proc = async {
        use tokio::io::AsyncWriteExt;
        while let Some(Ok(msg)) = stream.next().await {
            let bytes = match msg {
                Message::Text(t) => t.into_bytes(),
                Message::Binary(b) => b,
                Message::Close(_) => break,
                _ => continue,
            };
            if let Some(ref mut stdin) = stdin {
                if stdin.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        }
    };

    let from_out = async {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 4096];
        if let Some(ref mut out) = stdout {
            loop {
                match out.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if sink_out
                            .lock()
                            .await
                            .send(Message::Binary(buf[..n].to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    };

    let from_err = async {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 1024];
        if let Some(ref mut err) = stderr {
            loop {
                match err.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if sink_err
                            .lock()
                            .await
                            .send(Message::Binary(buf[..n].to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    };

    tokio::select! {
        _ = to_proc => {}
        _ = from_out => {}
        _ = from_err => {}
    }
    let _ = child.kill().await;
}
