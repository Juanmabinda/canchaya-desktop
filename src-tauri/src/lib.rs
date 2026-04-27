use std::path::PathBuf;
use std::sync::Mutex;
use serde::Deserialize;
use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

// SERVER_URL es donde el Rust (no la WebView) postea el grant para canjearlo
// por el agent_token. Se setea en build via env var CANCHAYA_SERVER_URL.
// Default: prod. Para staging: `CANCHAYA_SERVER_URL=https://staging.canchaya.ar`.
const SERVER_URL: &str = match option_env!("CANCHAYA_SERVER_URL") {
    Some(v) => v,
    None => "https://canchaya.ar",
};
const TOKEN_FILE: &str = "agent_token.txt";

struct AgentState {
    child: Mutex<Option<CommandChild>>,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    token: String,
}

fn token_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    Ok(dir.join(TOKEN_FILE))
}

fn read_token(app: &AppHandle) -> Option<String> {
    let path = token_path(app).ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn write_token(app: &AppHandle, token: &str) -> Result<(), String> {
    let path = token_path(app)?;
    std::fs::write(path, token).map_err(|e| format!("write: {e}"))
}

fn delete_token(app: &AppHandle) {
    if let Ok(path) = token_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

fn spawn_agent_if_token(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<AgentState>();
    if state.child.lock().unwrap().is_some() {
        return Ok(true);
    }

    let Some(token) = read_token(app) else {
        return Ok(false);
    };

    let sidecar = app
        .shell()
        .sidecar("canchaya-print")
        .map_err(|e| format!("sidecar: {e}"))?
        .env("CANCHAYA_AGENT_MANAGED", "1")
        .env("CANCHAYA_AGENT_TOKEN", &token)
        // El sidecar usa CANCHAYA_URL para WS + config endpoint. Lo
        // matcheamos al SERVER_URL del wrapper para que prod y staging
        // queden coherentes (token canjeado en staging conecta a staging).
        .env("CANCHAYA_URL", SERVER_URL);

    let (mut rx, child) = sidecar.spawn().map_err(|e| format!("spawn: {e}"))?;

    {
        let mut slot = state.child.lock().unwrap();
        *slot = Some(child);
    }

    let app_for_drain = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    println!("[agent] {}", String::from_utf8_lossy(&line).trim_end());
                }
                CommandEvent::Stderr(line) => {
                    eprintln!("[agent] {}", String::from_utf8_lossy(&line).trim_end());
                }
                CommandEvent::Terminated(payload) => {
                    eprintln!("[agent] terminated: code={:?}", payload.code);
                    if let Some(state) = app_for_drain.try_state::<AgentState>() {
                        *state.child.lock().unwrap() = None;
                    }
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(true)
}

fn kill_agent(app: &AppHandle) {
    if let Some(state) = app.try_state::<AgentState>() {
        if let Some(child) = state.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

#[tauri::command]
async fn pair_agent(app: AppHandle, grant: String) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(format!("{SERVER_URL}/api/desktop_agent/exchange"))
        .json(&serde_json::json!({ "grant": grant }))
        .send()
        .await
        .map_err(|e| format!("exchange request: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("exchange failed: HTTP {status}"));
    }

    let body: ExchangeResponse = response
        .json()
        .await
        .map_err(|e| format!("exchange parse: {e}"))?;

    write_token(&app, &body.token)?;

    kill_agent(&app);
    spawn_agent_if_token(&app)?;
    Ok(())
}

#[tauri::command]
fn agent_paired(app: AppHandle) -> bool {
    read_token(&app).is_some()
}

#[tauri::command]
fn agent_unpair(app: AppHandle) -> Result<(), String> {
    kill_agent(&app);
    delete_token(&app);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AgentState {
            child: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            pair_agent,
            agent_paired,
            agent_unpair
        ])
        .setup(|app| {
            let _ = spawn_agent_if_token(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                kill_agent(app);
            }
        });
}
