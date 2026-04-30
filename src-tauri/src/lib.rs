use std::path::PathBuf;
use std::sync::Mutex;
use serde::Deserialize;
use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_updater::UpdaterExt;

// SERVER_URL es donde el Rust (no la WebView) postea el grant para canjearlo
// por el agent_token. Se setea en build via env var CANCHAYA_SERVER_URL.
// Default: prod. Para staging: `CANCHAYA_SERVER_URL=https://staging.canchaya.ar`.
const SERVER_URL: &str = match option_env!("CANCHAYA_SERVER_URL") {
    Some(v) => v,
    None => "https://canchaya.ar",
};
const TOKEN_FILE: &str = "agent_token.txt";
const KIOSK_FILE: &str = "kiosk_mode";

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

// ─── Modo kiosko ────────────────────────────────────────────────
// Pref persistida en app_data_dir/kiosk_mode (contenido "1" = on).
// Aplicada al main window en setup. Toggleable desde JS via comando
// set_kiosk_mode + reabrir la app.

fn kiosk_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(KIOSK_FILE))
}

fn read_kiosk_mode(app: &AppHandle) -> bool {
    kiosk_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

fn write_kiosk_mode(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let path = kiosk_path(app).ok_or("no app_data_dir")?;
    std::fs::write(path, if enabled { "1" } else { "0" })
        .map_err(|e| format!("write kiosk: {e}"))
}

fn apply_kiosk_mode_if_set(app: &AppHandle) {
    if !read_kiosk_mode(app) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_decorations(false);
        let _ = window.set_fullscreen(true);
    }
}

#[tauri::command]
fn kiosk_mode_enabled(app: AppHandle) -> bool {
    read_kiosk_mode(&app)
}

#[tauri::command]
fn set_kiosk_mode(app: AppHandle, enabled: bool) -> Result<(), String> {
    write_kiosk_mode(&app, enabled)
    // No aplicamos inmediatamente — el set_decorations en runtime no
    // siempre matchea exit-fullscreen limpio. Pedimos restart de la app
    // (el JS muestra modal con "Cerrá y volvé a abrir CanchaYa POS").
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

// Cuando llega un deep link `canchaya://auth/callback?token=X` (emitido por
// el server tras OAuth exitoso), navegamos el WebView a /native/auth/session
// que canjea el token por una sesion Devise (mismo endpoint que iOS nativo).
fn register_oauth_deep_link(app: AppHandle) {
    let app_for_handler = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            // Aceptamos canchaya://auth/callback?token=X (prod) y
            // canchaya-staging://auth/callback?token=X (staging). El esquema
            // se valida via tauri.conf, pero matcheamos host="auth" + path.
            // Comparten el mismo handler porque la unica diferencia es el
            // SERVER_URL y eso ya esta baked en el binario via option_env!.
            let scheme_ok = url
                .scheme()
                .starts_with("canchaya");
            if !scheme_ok {
                continue;
            }
            if url.host_str().map(|h| h == "auth").unwrap_or(false)
                && url.path().starts_with("/callback")
            {
                if let Some((_, token)) = url
                    .query_pairs()
                    .find(|(k, _)| k == "token")
                {
                    let dest = format!(
                        "{SERVER_URL}/native/auth/session?token={}",
                        urlencoding::encode(&token)
                    );
                    if let Some(window) = app_for_handler.get_webview_window("main") {
                        if let Ok(parsed) = dest.parse() {
                            let _ = window.navigate(parsed);
                        }
                    }
                }
            }
        }
    });
}

// Best-effort: chequea updates al boot y los aplica silenciosamente. Si no
// hay manifest, falla la red, o el endpoint no existe todavia, lo logueamos
// y seguimos. La app vieja sigue funcionando — peor caso, no se actualiza.
fn check_for_updates(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(u) => u,
            Err(e) => {
                eprintln!("[updater] init failed: {e}");
                return;
            }
        };
        match updater.check().await {
            Ok(Some(update)) => {
                println!("[updater] update available: {}", update.version);
                if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
                    eprintln!("[updater] download/install failed: {e}");
                } else {
                    println!("[updater] installed; restart will apply it");
                }
            }
            Ok(None) => println!("[updater] up to date"),
            Err(e) => eprintln!("[updater] check failed: {e}"),
        }
    });
}

#[tauri::command]
fn agent_unpair(app: AppHandle) -> Result<(), String> {
    kill_agent(&app);
    delete_token(&app);
    Ok(())
}

// Bridge para descargas dentro del wrapper. canchaya.ar ya tiene helpers
// (downloadCanvasAsImage, shareOrDownloadUrl) que dependen de <a download>
// y Web Share API; ninguno funciona en WKWebView de Tauri. Aca abrimos un
// Save dialog nativo y escribimos los bytes que mando JS.
//
// Devuelve true si se guardo, false si el user cancelo.
// Abre la URL OAuth del provider en el navegador default del user. Asi
// Google/Apple ven a Safari/Chrome real (con sesiones, passkeys, password
// managers) en vez del WKWebView de Tauri donde fallan por anti-embedded
// browser. La OAuth callback termina en `canchaya://auth/callback?token=X`
// que vuelve al wrapper via deep-link.
#[tauri::command]
async fn open_oauth_in_browser(app: AppHandle, provider: String) -> Result<(), String> {
    let allowed = ["google_oauth2", "apple"];
    if !allowed.contains(&provider.as_str()) {
        return Err(format!("provider invalido: {provider}"));
    }
    let url = format!("{SERVER_URL}/users/auth/{provider}?origin=desktop");
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("opener: {e}"))
}

#[tauri::command]
async fn save_file_bytes(
    app: AppHandle,
    filename: String,
    bytes: Vec<u8>,
) -> Result<bool, String> {
    let dialog = app.dialog().clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog.file().set_file_name(&filename).blocking_save_file()
    })
    .await
    .map_err(|e| format!("save dialog: {e}"))?;

    let Some(path) = path else { return Ok(false) };
    // FilePath puede ser ruta o URI; en desktop siempre tenemos path real.
    let path_buf = path
        .into_path()
        .map_err(|e| format!("path resolve: {e}"))?;
    std::fs::write(&path_buf, bytes).map_err(|e| format!("write {}: {e}", path_buf.display()))?;
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(AgentState {
            child: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            pair_agent,
            agent_paired,
            agent_unpair,
            save_file_bytes,
            open_oauth_in_browser,
            kiosk_mode_enabled,
            set_kiosk_mode
        ])
        .setup(|app| {
            apply_kiosk_mode_if_set(app.handle());
            let _ = spawn_agent_if_token(app.handle());
            check_for_updates(app.handle().clone());
            register_oauth_deep_link(app.handle().clone());

            // Inyectar version del Desktop POS en el WebView en cada navegacion
            // para que la app Rails pueda mostrarla cerca del logo.
            // on_page_load corre tanto al cargar la URL inicial como en cualquier
            // navegacion turbo/anchor — asi siempre esta disponible.
            let version = app.package_info().version.to_string();
            if let Some(window) = app.get_webview_window("main") {
                let v = version.clone();
                window.on_page_load(move |w, _payload| {
                    let script = format!(
                        "window.__CANCHAYA_DESKTOP_VERSION__ = {:?}; document.dispatchEvent(new CustomEvent('canchaya:desktop-version', {{ detail: {:?} }}));",
                        v, v
                    );
                    let _ = w.eval(&script);
                });
            }

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
