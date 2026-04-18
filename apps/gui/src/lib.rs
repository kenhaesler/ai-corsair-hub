mod commands;
mod dto;
mod hardware_thread;
mod state;

use std::sync::OnceLock;

use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use corsair_common::config::AppConfig;
use corsair_fancontrol::control_loop;

use state::AppState;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

/// Retention: keep the most recent N daily log files. Older files are pruned
/// at startup to prevent unbounded disk growth.
const LOG_RETENTION_DAYS: usize = 14;

/// Non-blocking log writer guard. Must live for the full process lifetime,
/// otherwise the background flushing thread is dropped and queued log lines
/// are silently lost. Stored in a OnceLock so it's never dropped until exit.
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn run() {
    init_tracing();

    info!("corsair-hub GUI starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus existing window if user tries to launch again
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let config = load_config_or_default();
            let hw_sender = hardware_thread::spawn(config, app.handle().clone());
            app.manage(AppState { hw_sender });

            // --- System tray ---
            let silent =
                MenuItem::with_id(app, "silent", "Silent", true, None::<&str>)?;
            let balanced =
                MenuItem::with_id(app, "balanced", "Balanced", true, None::<&str>)?;
            let performance =
                MenuItem::with_id(app, "performance", "Performance", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let open =
                MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&silent, &balanced, &performance, &sep, &open, &quit],
            )?;

            TrayIconBuilder::new()
                .tooltip("Corsair Hub")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "silent" | "balanced" | "performance" => {
                            let state = app.state::<AppState>();
                            let (tx, _rx) = tokio::sync::oneshot::channel();
                            let _ = state.hw_sender.send(
                                crate::state::HwCommand::ApplyPreset {
                                    preset: id.to_string(),
                                    reply: tx,
                                },
                            );
                            info!("Tray preset applied: {}", id);
                        }
                        "open" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => {
                            let state = app.state::<AppState>();
                            let _ =
                                state.hw_sender.send(crate::state::HwCommand::Shutdown);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide to tray instead of closing
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_devices,
            commands::get_config,
            commands::save_config,
            commands::apply_preset,
            #[allow(deprecated)]
            commands::set_manual_duty,
            commands::set_manual_duty_by_device_id,
            commands::rename_device,
            commands::validate_config,
            commands::set_rgb_config,
            commands::set_rgb_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Config directory: %APPDATA%\corsair-hub (outside Tauri's watched dirs).
pub(crate) fn config_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    std::path::PathBuf::from(app_data)
        .join("corsair-hub")
        .join("config.toml")
}

/// Log directory: %APPDATA%\corsair-hub\logs (alongside config.toml).
/// Separate dir so log rotation/pruning never touches user configuration.
fn log_dir_path() -> std::path::PathBuf {
    config_path()
        .parent()
        .map(|p| p.join("logs"))
        .unwrap_or_else(|| std::path::PathBuf::from("logs"))
}

/// Initialize tracing with a stderr layer and a daily-rolling file layer under
/// `%APPDATA%\corsair-hub\logs\corsair-hub.log`.
///
/// The file writer is non-blocking: log calls enqueue to a bounded channel
/// that the background thread drains to disk. Under disk pressure the channel
/// drops overflow rather than blocking the caller — acceptable trade because
/// the stderr layer still captures everything and hardware-thread stalls would
/// be catastrophic for fan control.
///
/// The WorkerGuard is stashed in a process-lifetime OnceLock. Dropping it
/// would stop the background thread mid-flight and lose queued messages.
///
/// As a best-effort housekeeping step, log files older than the most recent
/// LOG_RETENTION_DAYS are pruned at startup.
fn init_tracing() {
    let log_dir = log_dir_path();
    let dir_ready = std::fs::create_dir_all(&log_dir).is_ok();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("corsair=info,corsair_hub_gui=info,corsair_fancontrol=info,corsair_hid=info")
    });

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false);

    if dir_ready {
        let file_appender = tracing_appender::rolling::daily(&log_dir, "corsair-hub.log");
        let (nb_writer, guard) = tracing_appender::non_blocking(file_appender);
        // Ignore set error: init_tracing is only ever called once from run().
        let _ = LOG_GUARD.set(guard);

        let file_layer = fmt::layer()
            .with_writer(nb_writer)
            .with_ansi(false)
            .with_target(true);

        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();

        info!(log_dir = %log_dir.display(), "Logging initialized (stderr + daily file)");
        prune_old_logs(&log_dir, LOG_RETENTION_DAYS);
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .init();
        tracing::warn!(
            log_dir = %log_dir.display(),
            "Could not create log directory — falling back to stderr only"
        );
    }
}

/// Best-effort retention prune: keep the `keep` most-recently-modified log
/// files in `dir`, delete the rest. Only operates on files whose name starts
/// with "corsair-hub.log" (so it never touches unrelated user files in case
/// the log dir is shared or the path is somehow aliased).
fn prune_old_logs(dir: &std::path::Path, keep: usize) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = entries
        .filter_map(|res| res.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("corsair-hub.log")
        })
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let mtime = meta.modified().ok()?;
            Some((entry.path(), mtime))
        })
        .collect();

    if files.len() <= keep {
        return;
    }

    files.sort_by(|a, b| b.1.cmp(&a.1)); // newest first

    for (path, _) in files.iter().skip(keep) {
        match std::fs::remove_file(path) {
            Ok(()) => tracing::debug!(path = %path.display(), "Pruned old log file"),
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "Failed to prune old log"),
        }
    }
}

fn load_config_or_default() -> AppConfig {
    let path = config_path();

    // Also try legacy location (cwd/config.toml) for migration
    let legacy_path = std::env::current_dir()
        .unwrap_or_default()
        .join("config.toml");

    for p in [&path, &legacy_path] {
        if p.exists() {
            match control_loop::load_config(p) {
                Ok(config) => {
                    info!("Loaded config from {}", p.display());
                    return config;
                }
                Err(e) => {
                    tracing::warn!("Failed to load config from {}: {}", p.display(), e);
                }
            }
        }
    }

    info!("Using default config");
    AppConfig::default()
}
