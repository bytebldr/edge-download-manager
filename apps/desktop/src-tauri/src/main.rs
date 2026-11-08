// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use edm_core::adaptive::AdaptiveEngine;
use edm_ipc::DownloadState;
use edm_store::Store;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use uuid::Uuid;

pub mod commands;
pub mod features;
pub mod lifecycle;
pub mod os;
pub mod services;
pub mod state;
pub mod updater;
pub mod ws;
pub mod ws_server;

pub use commands::*;
pub use os::*;
pub use services::*;
pub use state::*;
pub use ws::*;

fn init_logging() {
    let log_dir = match std::env::var("APPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("logs"),
        Err(_) => std::env::temp_dir().join("EDM").join("logs"),
    };
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "edm.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(_guard));

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);

    // Register panic hook
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info)
        };
        tracing::error!("PANIC DETECTED: {} at {:?}", msg, info.location());
        eprintln!("PANIC DETECTED: {} at {:?}", msg, info.location());
    }));
}

fn get_db_path() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data).join("EDM");
        let _ = std::fs::create_dir_all(&p);
        p.join("store.db")
    } else {
        std::env::temp_dir().join("edm_store.db")
    }
}

fn open_persistent_store() -> Store {
    let path = get_db_path();
    Store::open(&path).unwrap_or_else(|_| Store::open_in_memory().unwrap())
}

fn main() {
    let t_boot = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--clean-uninstall" || a == "--purge-data") {
        #[cfg(windows)]
        purge_all_user_data();
        return;
    }

    init_logging();
    tracing::info!("EDM starting up. Args: {:?}", args);

    #[cfg(windows)]
    set_app_user_model_id();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    let rt_handle = rt.handle().clone();

    let t_db = std::time::Instant::now();
    let store = open_persistent_store();
    tracing::info!("[PERF] Persistent store opened in {:?}", t_db.elapsed());
    let engine = Arc::new(AdaptiveEngine::new(store.clone()));

    let token = ws_server::get_or_create_token();
    let (create_task_tx, mut create_task_rx) =
        tokio::sync::mpsc::channel::<ws_server::CreateTaskCommand>(64);
    let (action_tx, mut action_rx) = tokio::sync::mpsc::channel::<ws_server::ActionCommand>(64);
    let (refresh_task_tx, mut refresh_task_rx) =
        tokio::sync::mpsc::channel::<ws_server::RefreshTaskCommand>(64);
    let (probe_media_tx, mut probe_media_rx) =
        tokio::sync::mpsc::channel::<ws_server::ProbeMediaCommand>(64);

    let ws_manager = Arc::new(ws_server::WsManager::new(
        token,
        ws_server::EXTENSION_ID.to_string(),
        store.clone(),
        create_task_tx,
        action_tx,
        refresh_task_tx,
        probe_media_tx,
    ));

    let lifecycle = Arc::new(lifecycle::LifecycleManager::new(
        store.clone(),
        ws_manager.clone(),
    ));

    let power_countdown = Arc::new(features::PowerCountdownController::new());
    let global_limiter = Arc::new(edm_core::TokenBucketLimiter::new(0));

    // Startup crash recovery: Reset interrupted 'Downloading' or 'Probing' tasks to 'Paused'
    if let Ok(records) = store.list_all_downloads() {
        for r in records {
            if r.state == DownloadState::Downloading || r.state == DownloadState::Probing {
                let _ = store.update_download_state(
                    r.id,
                    DownloadState::Paused,
                    Some("Interrupted by application shutdown"),
                );
            }
        }
    }

    let app_state = AppState {
        store,
        engine,
        tasks: Arc::new(Mutex::new(HashMap::new())),
        rt_handle: rt_handle.clone(),
        ws_manager: ws_manager.clone(),
        tray_handle: Arc::new(Mutex::new(None)),
        app_handle: Arc::new(Mutex::new(None)),
        lifecycle: lifecycle.clone(),
        power_countdown,
        global_limiter,
    };

    // Fast-path: Start WebSocket Server on 127.0.0.1:14370 FIRST
    let ws_mgr_srv = ws_manager.clone();
    let t_ws = t_boot;
    rt_handle.spawn(async move {
        ws_mgr_srv.sync_from_store().await;
        tracing::info!("[PERF] Port 14370 ready in {:?}", t_ws.elapsed());
        ws_server::start_ws_server(ws_mgr_srv).await;
    });

    spawn_extension_bridge(app_state.clone(), rt_handle.clone());

    // Register Native Messaging Host and provision extension files immediately on boot
    #[cfg(windows)]
    {
        register_native_messaging_host();
        let _ = prepare_extension_files_internal();
    }

    // Background Queue Scheduler loop (evaluates every 10 seconds)
    let queue_state = app_state.clone();
    rt_handle.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            if let Ok(queues) = queue_state.store.list_queues() {
                let now = chrono::Local::now();
                for q in queues {
                    if features::is_queue_active_at_time(&q, now) {
                        let active_count = {
                            if let Ok(tasks) = queue_state.tasks.lock() {
                                tasks.values().filter(|h| !h.cancelled.load(Ordering::Relaxed)).count()
                            } else {
                                0
                            }
                        };
                        let max_allowed = (q.max_concurrent as usize).max(1);
                        if active_count < max_allowed {
                            if let Ok(records) = queue_state.store.list_all_downloads() {
                                for r in records {
                                    if r.state == DownloadState::Queued {
                                        let _ = execute_download(
                                            &queue_state,
                                            r.id,
                                            r.url,
                                            Some(r.filename),
                                            Some(PathBuf::from(r.save_dir)),
                                            Some(r.connections),
                                            r.size_total,
                                            None,
                                            r.referer,
                                            r.user_agent,
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // Background lifecycle monitoring loop (checks every 15 seconds)
    let lc_monitor = lifecycle.clone();
    let tasks_monitor = app_state.tasks.clone();
    rt_handle.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let active_count = {
                let mut count = 0;
                if let Ok(tasks) = tasks_monitor.lock() {
                    for handle in tasks.values() {
                        if !handle.cancelled.load(Ordering::Relaxed) {
                            count += 1;
                        }
                    }
                }
                count
            };
            lc_monitor.check_lifecycle_step(active_count, false).await;
        }
    });

    // Inbound WebSocket Task Creator Loop
    let create_state = app_state.clone();
    rt_handle.spawn(async move {
        while let Some(cmd) = create_task_rx.recv().await {
            let cookie_header = if !cmd.cookies.is_empty() {
                Some(
                    cmd.cookies
                        .iter()
                        .map(|c| format!("{}={}", c.name, c.value))
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            } else {
                None
            };
            let id = Uuid::new_v4();
            let res = execute_download(
                &create_state,
                id,
                cmd.url,
                cmd.filename,
                None,
                None,
                None,
                cookie_header,
                cmd.referrer,
                cmd.user_agent,
            );
            let _ = cmd.resp_tx.send(res.map(|u| u.to_string()));
        }
    });

    // Inbound WebSocket Task Actions Loop
    let action_state = app_state.clone();
    rt_handle.spawn(async move {
        while let Some(cmd) = action_rx.recv().await {
            let res = match cmd.action.as_str() {
                "pause" => {
                    if let Ok(id_uuid) = Uuid::parse_str(&cmd.id) {
                        {
                            let tasks = action_state.tasks.lock().unwrap();
                            if let Some(t) = tasks.get(&cmd.id) {
                                t.cancelled.store(true, Ordering::Relaxed);
                            }
                        }
                        let _ = action_state.store.update_download_state(
                            id_uuid,
                            DownloadState::Paused,
                            None,
                        );
                        action_state
                            .ws_manager
                            .update_task_progress(&cmd.id, 0, None, 0, "paused")
                            .await;
                        Ok(())
                    } else {
                        Err("Invalid task id".into())
                    }
                }
                "resume" => {
                    if let Ok(id_uuid) = Uuid::parse_str(&cmd.id) {
                        let records = action_state.store.list_all_downloads().unwrap_or_default();
                        if let Some(r) = records.into_iter().find(|d| d.id == id_uuid) {
                            let _ = execute_download(
                                &action_state,
                                id_uuid,
                                r.url,
                                Some(r.filename),
                                Some(PathBuf::from(r.save_dir)),
                                Some(r.connections),
                                r.size_total,
                                None,
                                r.referer,
                                r.user_agent,
                            );
                            let _ = action_state.store.update_download_state(
                                id_uuid,
                                DownloadState::Downloading,
                                None,
                            );
                            action_state
                                .ws_manager
                                .update_task_progress(
                                    &cmd.id,
                                    r.size_done,
                                    r.size_total,
                                    0,
                                    "downloading",
                                )
                                .await;
                            Ok(())
                        } else {
                            Err("Task not found".into())
                        }
                    } else {
                        Err("Invalid task id".into())
                    }
                }
                "cancel" => {
                    if let Ok(id_uuid) = Uuid::parse_str(&cmd.id) {
                        {
                            let tasks = action_state.tasks.lock().unwrap();
                            if let Some(t) = tasks.get(&cmd.id) {
                                t.cancelled.store(true, Ordering::Relaxed);
                            }
                        }
                        let _ = action_state.store.update_download_state(
                            id_uuid,
                            DownloadState::Cancelled,
                            Some("Cancelled by user"),
                        );
                        action_state
                            .ws_manager
                            .update_task_progress(&cmd.id, 0, None, 0, "failed")
                            .await;
                        Ok(())
                    } else {
                        Err("Invalid task id".into())
                    }
                }
                "remove" => {
                    let delete_file = cmd.delete_file;
                    remove_download_task(&action_state, &cmd.id, delete_file).await
                }
                "open_file" => {
                    let live_tasks = action_state.ws_manager.get_live_tasks();
                    let live = live_tasks.read().await;
                    if let Some(t) = live.get(&cmd.id) {
                        ws_server::secure_open_file(&t.save_dir, &t.filename, &t.status)
                    } else if let Ok(id_uuid) = Uuid::parse_str(&cmd.id) {
                        let records = action_state.store.list_all_downloads().unwrap_or_default();
                        if let Some(r) = records.into_iter().find(|d| d.id == id_uuid) {
                            let status = match r.state {
                                DownloadState::Completed => "completed",
                                _ => "downloading",
                            };
                            ws_server::secure_open_file(
                                &PathBuf::from(r.save_dir),
                                &r.filename,
                                status,
                            )
                        } else {
                            Err("Task not found".into())
                        }
                    } else {
                        Err("Task not found".into())
                    }
                }
                "open_folder" => {
                    let live_tasks = action_state.ws_manager.get_live_tasks();
                    let live = live_tasks.read().await;
                    if let Some(t) = live.get(&cmd.id) {
                        ws_server::secure_open_folder(&t.save_dir, &t.filename)
                    } else if let Ok(id_uuid) = Uuid::parse_str(&cmd.id) {
                        let records = action_state.store.list_all_downloads().unwrap_or_default();
                        if let Some(r) = records.into_iter().find(|d| d.id == id_uuid) {
                            ws_server::secure_open_folder(&PathBuf::from(r.save_dir), &r.filename)
                        } else {
                            Err("Task not found".into())
                        }
                    } else {
                        Err("Task not found".into())
                    }
                }
                _ => Err("Unsupported action".into()),
            };
            let _ = cmd.resp_tx.send(res);
        }
    });

    // Inbound WebSocket Task Refresh Loop
    let refresh_state = app_state.clone();
    rt_handle.spawn(async move {
        while let Some(cmd) = refresh_task_rx.recv().await {
            if let Ok(id_uuid) = Uuid::parse_str(&cmd.task_id) {
                let res = features::validate_and_refresh_link(
                    &refresh_state.store,
                    id_uuid,
                    &cmd.new_url,
                    cmd.headers.as_ref(),
                )
                .await;
                match res {
                    Ok(task) => {
                        let _ = execute_download(
                            &refresh_state,
                            task.id,
                            task.url,
                            Some(task.filename),
                            Some(PathBuf::from(task.save_dir)),
                            Some(task.connections),
                            task.size_total,
                            None,
                            task.referer,
                            task.user_agent,
                        );
                        let _ = cmd.resp_tx.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = cmd.resp_tx.send(Err(e));
                    }
                }
            } else {
                let _ = cmd.resp_tx.send(Err("Invalid task UUID".into()));
            }
        }
    });

    // Inbound WebSocket Media Prober Loop
    rt_handle.spawn(async move {
        while let Some(cmd) = probe_media_rx.recv().await {
            let res = features::probe_media(&cmd.media_url).await;
            let _ = cmd.resp_tx.send(res.map(|r| (r.variants, r.is_drm)));
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let is_autostart = argv.iter().any(|arg| arg == "--autostart");
            let is_background = argv.iter().any(|arg| arg == "--background" || arg == "--listener-only");
            let show_window = argv.iter().any(|arg| arg == "--show-window");

            let state = app.state::<AppState>();
            let lc = state.lifecycle.clone();

            if !is_autostart && (!is_background || show_window) {
                state.rt_handle.spawn(async move {
                    lc.record_wake(true).await;
                });
            } else {
                state.rt_handle.spawn(async move {
                    lc.record_wake(false).await;
                });
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let app_state_ref = app.state::<AppState>();
            app_state_ref.lifecycle.set_app_handle(app.handle().clone());

            let args: Vec<String> = std::env::args().collect();
            let is_autostart_boot = args.iter().any(|arg| arg == "--autostart");
            let is_background_boot = args.iter().any(|arg| arg == "--background" || arg == "--listener-only");
            let show_window = args.iter().any(|arg| arg == "--show-window");

            if let Some(window) = app.get_webview_window("main") {
                if let Ok(icon) =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
                {
                    let _ = window.set_icon(icon);
                }
                let _ = window.center();

                if (is_autostart_boot || is_background_boot) && !show_window {
                    tracing::info!(
                        "EDM started in background mode; destroying window to minimize memory footprint"
                    );
                    let _ = window.destroy();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                    tracing::info!(
                        "Main desktop window successfully loaded: {:?}",
                        window.url()
                    );
                }
            }

            // Tray Icon Setup
            let open_i = MenuItem::with_id(app, "open", "Open EDM", true, None::<&str>)?;
            let pause_all_i = MenuItem::with_id(app, "pause_all", "Pause all", true, None::<&str>)?;
            let resume_all_i =
                MenuItem::with_id(app, "resume_all", "Resume all", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let tray_menu = Menu::with_items(
                app,
                &[&open_i, &pause_all_i, &resume_all_i, &settings_i, &quit_i],
            )?;

            let tray_icon_bytes = include_bytes!("../icons/icon.ico");
            let icon_img = tauri::image::Image::from_bytes(tray_icon_bytes).map_err(|e| {
                tauri::Error::Anyhow(anyhow::anyhow!("Failed to parse icon.ico: {}", e))
            })?;

            let tray = TrayIconBuilder::new()
                .icon(icon_img)
                .tooltip("EDM — Edge Download Manager")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        let state = app.state::<AppState>();
                        let lc = state.lifecycle.clone();
                        state.rt_handle.spawn(async move {
                            lc.record_wake(true).await;
                        });
                    }
                    "pause_all" => {
                        let state = app.state::<AppState>();
                        let tasks = state.tasks.lock().unwrap();
                        for handle in tasks.values() {
                            handle.cancelled.store(true, Ordering::SeqCst);
                        }
                    }
                    "resume_all" => {
                        let state = app.state::<AppState>();
                        if let Ok(records) = state.store.list_all_downloads() {
                            for r in records {
                                if r.state == DownloadState::Paused {
                                    let _ = execute_download(
                                        &state,
                                        r.id,
                                        r.url,
                                        Some(r.filename),
                                        Some(PathBuf::from(r.save_dir)),
                                        Some(r.connections),
                                        r.size_total,
                                        None,
                                        r.referer,
                                        r.user_agent,
                                    );
                                    let _ = state.store.update_download_state(
                                        r.id,
                                        DownloadState::Downloading,
                                        None,
                                    );
                                }
                            }
                        }
                    }
                    "settings" => {
                        let state = app.state::<AppState>();
                        let lc = state.lifecycle.clone();
                        state.rt_handle.spawn(async move {
                            lc.record_wake(true).await;
                        });
                    }
                    "quit" => {
                        let state = app.state::<AppState>();
                        trigger_graceful_quit(app.clone(), state.inner().clone());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let state = tray.app_handle().state::<AppState>();
                        let lc = state.lifecycle.clone();
                        state.rt_handle.spawn(async move {
                            lc.record_wake(true).await;
                        });
                    }
                })
                .build(app)?;

            let app_state_ref = app.state::<AppState>();
            *app_state_ref.tray_handle.lock().unwrap() = Some(tray.clone());
            *app_state_ref.app_handle.lock().unwrap() = Some(app.handle().clone());

            // Initialize theme and palette in ws_manager from store
            if let Ok(Some(settings_str)) = app_state_ref.store.get_setting("app_settings") {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&settings_str) {
                    let theme = val
                        .get("general")
                        .and_then(|g| g.get("theme"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let palette = val
                        .get("general")
                        .and_then(|g| g.get("palette"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let ws_mgr = app_state_ref.ws_manager.clone();
                    app_state_ref.rt_handle.spawn(async move {
                        ws_mgr.update_theme_palette(theme, palette).await;
                    });
                }
            }

            // Background dynamic tray tooltip ticker (every 1 second)
            let ticker_tasks = app_state_ref.tasks.clone();
            let ticker_tray = tray.clone();
            app_state_ref.rt_handle.spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let (active_count, agg_bps) = {
                        let mut count = 0;
                        let mut speed = 0u64;
                        if let Ok(tasks) = ticker_tasks.lock() {
                            for h in tasks.values() {
                                if !h.cancelled.load(Ordering::Relaxed) {
                                    count += 1;
                                    if let Ok(lock) = h.telemetry.lock() {
                                        if let Some(ref telem) = *lock {
                                            speed += telem.rolling_throughput_bps;
                                        }
                                    }
                                }
                            }
                        }
                        (count, speed)
                    };

                    let tooltip = if active_count == 0 {
                        "EDM — All downloads complete".to_string()
                    } else {
                        let mbps = (agg_bps as f64) / (1024.0 * 1024.0);
                        format!("EDM — {} downloading ({:.1} MB/s)", active_count, mbps)
                    };

                    let _ = ticker_tray.set_tooltip(Some(tooltip));
                }
            });

            Ok(())
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            probe_url,
            start_download,
            pause_download,
            resume_download,
            cancel_download,
            delete_download,
            list_downloads,
            open_file_path,
            open_dir_path,
            get_download_telemetry,
            get_stats,
            get_feature_packs,
            get_settings,
            save_settings,
            set_speed_limit,
            get_extension_token,
            app_minimize,
            app_toggle_maximize,
            app_close,
            app_start_dragging,
            updater::get_app_version,
            updater::check_for_updates,
            updater::install_update,
            get_lifecycle_status,
            wake_engine,
            get_browser_diagnostics,
            repair_native_host_registration,
            detect_installed_browsers,
            prepare_extension_files,
            get_extension_folder_path,
            remove_extension_folder,
            open_browser_url,
            get_firefox_xpi_url,
            is_wizard_completed,
            set_wizard_completed,
            get_system_downloads_dir,
            select_download_directory,
            get_disk_free_space,
            refresh_download_link,
            expand_batch_urls,
            add_batch_downloads,
            check_duplicate_download,
            verify_file_checksum,
            get_history,
            clear_history,
            export_history_csv,
            get_queues,
            save_queue,
            delete_queue,
            cancel_power_countdown,
            get_power_countdown_status,
            probe_media_page
        ])
        .build(tauri::generate_context!())
        .expect("error while building EDM desktop application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_extension_files_integrity() {
        let res = prepare_extension_files_internal();
        assert!(res.is_ok(), "prepare_extension_files failed: {:?}", res);
        let path = PathBuf::from(res.unwrap());
        assert!(path.exists(), "Target folder should exist");
        assert!(path.join("manifest.json").exists(), "manifest.json should exist");
        assert!(path.join("hashes.json").exists(), "hashes.json should exist");
    }

    #[test]
    fn test_system_downloads_dir() {
        let dir = get_system_downloads_dir_internal();
        assert!(!dir.is_empty(), "System downloads directory should not be empty");
    }
}
