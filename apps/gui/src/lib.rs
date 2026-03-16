mod commands;
mod dto;
mod hardware_thread;
mod state;

use tracing::info;
use tracing_subscriber::EnvFilter;

use corsair_common::config::AppConfig;
use corsair_fancontrol::control_loop;

use state::AppState;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub fn run() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("corsair=info,corsair_hub_gui=info")),
        )
        .init();

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
            commands::set_manual_duty,
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
