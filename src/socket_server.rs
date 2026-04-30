//! Unix-socket endpoint for direct JSON-RPC 2.0 queries.
//!
//! Cap and other local clients use this endpoint to query mycelium_gain data
//! without spawning a subprocess. Bind path is
//! `~/.local/share/basidiocarp/mycelium/mycelium.sock`. The endpoint
//! descriptor at `~/.config/mycelium/mycelium.endpoint.json` lets clients
//! discover the socket path via the `local-service-endpoint-v1` convention.
//!
//! # Supported methods
//!
//! - `PING` / `ping` — health probe, returns `{}`
//! - `mycelium_gain` — token savings query; params mirror the `gain --format json` flags

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};
use tracing::{debug, error};

const CAPABILITY_ID: &str = "token.gain.v1";
const PING_METHOD: &str = "PING";

fn write_endpoint_descriptor(socket_path: &Path) -> Result<()> {
    let config_dir = spore::paths::config_dir("mycelium");
    std::fs::create_dir_all(&config_dir)?;
    let descriptor_path = config_dir.join("mycelium.endpoint.json");
    let descriptor = json!({
        "schema_version": "1.0",
        "transport": "unix-socket",
        "endpoint": socket_path.to_string_lossy(),
        "capability_id": CAPABILITY_ID,
        "version": env!("CARGO_PKG_VERSION"),
        "health_probe": { "method": PING_METHOD, "timeout_ms": 1000 }
    });
    std::fs::write(&descriptor_path, serde_json::to_string_pretty(&descriptor)?)?;
    Ok(())
}

fn remove_stale_socket(socket_path: &Path) {
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

fn ok_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() }
    })
}

fn write_response(writer: &mut (impl Write + ?Sized), response: &Value) {
    if let Ok(bytes) = serde_json::to_vec(response) {
        let _ = writer.write_all(&bytes);
        let _ = writer.write_all(b"\n");
        let _ = writer.flush();
    }
}

// ---------------------------------------------------------------------------
// mycelium_gain handler
// ---------------------------------------------------------------------------

fn handle_gain(params: &Value) -> Value {
    let daily = params.get("daily").and_then(|v| v.as_bool()).unwrap_or(false);
    let weekly = params.get("weekly").and_then(|v| v.as_bool()).unwrap_or(false);
    let monthly = params.get("monthly").and_then(|v| v.as_bool()).unwrap_or(false);
    let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let history = params.get("history").and_then(|v| v.as_bool()).unwrap_or(false);
    let limit = params.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(10);
    let project_path = params.get("project_path").and_then(|v| v.as_str()).map(str::to_owned);
    let projects = params.get("projects").and_then(|v| v.as_bool()).unwrap_or(false);

    let tracker = match crate::tracking::Tracker::new() {
        Ok(t) => t,
        Err(e) => return json!({ "error": format!("tracker init: {e}") }),
    };

    let result = if projects {
        crate::gain::gain_projects_json_string(&tracker)
    } else {
        crate::gain::gain_json_string(
            &tracker,
            daily,
            weekly,
            monthly,
            all,
            history,
            limit,
            project_path.as_deref(),
        )
    };

    match result {
        Ok(json_str) => serde_json::from_str(&json_str).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": format!("gain query: {e}") }),
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

fn handle_connection(stream: std::os::unix::net::UnixStream) {
    let writer_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            error!("failed to clone unix stream: {e}");
            return;
        }
    };
    let mut reader = BufReader::new(stream);
    let mut writer = writer_stream;

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return,
            Ok(_) => {}
            Err(e) => {
                error!("socket read error: {e}");
                return;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let resp = err_response(Value::Null, -32700, format!("parse error: {e}"));
                write_response(&mut writer, &resp);
                return;
            }
        };

        let id = match msg.get("id").cloned() {
            Some(id) if !id.is_null() => id,
            _ => continue, // notification — no response
        };

        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        debug!("socket request: {method}");

        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

        let response = match method {
            m if m == PING_METHOD || m == "ping" => ok_response(id, json!({})),
            "mycelium_gain" => {
                let result = handle_gain(&params);
                if result.get("error").is_some() {
                    let msg = result["error"].as_str().unwrap_or("gain error").to_string();
                    err_response(id, -32000, msg)
                } else {
                    ok_response(id, result)
                }
            }
            _ => err_response(id, -32601, format!("method not found: {method}")),
        };

        write_response(&mut writer, &response);
    }
}

// ---------------------------------------------------------------------------
// Server entry point
// ---------------------------------------------------------------------------

/// Start the mycelium unix-socket service endpoint.
///
/// Binds to `~/.local/share/basidiocarp/mycelium/mycelium.sock`, writes the
/// endpoint descriptor to `~/.config/mycelium/mycelium.endpoint.json`, then
/// accepts connections indefinitely. Each connection is handled in a
/// background thread.
pub fn run_socket_server(_compact: bool) -> Result<()> {
    let socket_path: PathBuf = spore::paths::data_dir("basidiocarp")
        .join("mycelium")
        .join("mycelium.sock");

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_stale_socket(&socket_path);

    let listener = std::os::unix::net::UnixListener::bind(&socket_path).map_err(|e| {
        anyhow::anyhow!(
            "failed to bind mycelium socket {}: {e}",
            socket_path.display()
        )
    })?;

    write_endpoint_descriptor(&socket_path)?;

    tracing::info!(
        socket = %socket_path.display(),
        "mycelium socket endpoint ready"
    );

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                std::thread::spawn(move || handle_connection(stream));
            }
            Err(e) => error!("mycelium socket accept error: {e}"),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use tempfile::TempDir;

    fn temp_socket_path(dir: &TempDir) -> PathBuf {
        dir.path().join("test.sock")
    }

    #[test]
    fn socket_server_ping_responds_ok() {
        let tmp = TempDir::new().unwrap();
        let socket_path = temp_socket_path(&tmp);

        remove_stale_socket(&socket_path);
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let socket_path_clone = socket_path.clone();

        let handle = std::thread::spawn(move || {
            if let Ok(stream) = listener.accept().map(|(s, _)| s) {
                handle_connection(stream);
            }
        });

        let mut client = std::os::unix::net::UnixStream::connect(&socket_path_clone).unwrap();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"PING","params":null}"#;
        client.write_all(request.as_bytes()).unwrap();
        client.write_all(b"\n").unwrap();
        client.flush().unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();

        let reader = BufReader::new(&client);
        let line = reader.lines().next().expect("response").unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], 1);
        assert!(v.get("result").is_some());
        assert!(v.get("error").is_none());

        handle.join().unwrap();
    }

    #[test]
    fn socket_server_unknown_method_returns_method_not_found() {
        let tmp = TempDir::new().unwrap();
        let socket_path = temp_socket_path(&tmp);

        remove_stale_socket(&socket_path);
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let socket_path_clone = socket_path.clone();

        let handle = std::thread::spawn(move || {
            if let Ok(stream) = listener.accept().map(|(s, _)| s) {
                handle_connection(stream);
            }
        });

        let mut client = std::os::unix::net::UnixStream::connect(&socket_path_clone).unwrap();
        let request = r#"{"jsonrpc":"2.0","id":2,"method":"no_such_method","params":{}}"#;
        client.write_all(request.as_bytes()).unwrap();
        client.write_all(b"\n").unwrap();
        client.flush().unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();

        let reader = BufReader::new(&client);
        let line = reader.lines().next().expect("response").unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], 2);
        assert!(v.get("error").is_some());
        assert_eq!(v["error"]["code"], -32601);

        handle.join().unwrap();
    }
}
