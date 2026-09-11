#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[macro_use]
extern crate log;

use std::sync::{Arc, Mutex};

use rqs_lib::channel::{ChannelDirection, ChannelMessage};
use rqs_lib::{EndpointInfo, SendInfo, State, Visibility, RQS};
use store::get_startminimized;
#[cfg(target_os = "macos")]
use tauri::image::Image;
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Window, WindowEvent,
};
use tauri_plugin_autostart::MacosLauncher;
use tokio::sync::{broadcast, mpsc, watch};

use crate::logger::set_up_logging;
use crate::notification::{send_request_notification, send_temporarily_notification};
use crate::store::{
    get_download_path, get_port, get_realclose, get_visibility, init_default, set_visibility,
};

mod cmds;
mod logger;
mod notification;
mod store;

pub struct AppState {
    pub message_sender: broadcast::Sender<ChannelMessage>,
    pub dch_sender: broadcast::Sender<EndpointInfo>,
    pub visibility_sender: Arc<Mutex<watch::Sender<Visibility>>>,
    pub sender_file: mpsc::Sender<SendInfo>,
    pub ble_receiver: broadcast::Receiver<()>,
    pub rqs: Mutex<RQS>,
    pub pending_files: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Define tauri async runtime to be tokio
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    // Build and run Tauri app
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            trace!("tauri_plugin_single_instance: instance already running");
            let files: Vec<String> = argv
                .into_iter()
                .skip(1)
                .map(|arg| arg.trim_matches('"').to_string())
                .filter(|arg| !arg.starts_with('-') && arg != "%1")
                .collect();
            if !files.is_empty() {
                app.state::<AppState>()
                    .pending_files
                    .lock()
                    .unwrap()
                    .extend(files.clone());
                let _ = app.emit("send_files", files);
            }
            open_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            cmds::change_download_path,
            cmds::change_visibility,
            cmds::start_discovery,
            cmds::stop_discovery,
            cmds::get_hostname,
            cmds::send_payload,
            cmds::send_to_rs,
            take_pending_files,
        ])
        .setup(|app| {
            // Setting up logging inside file for the app
            set_up_logging(app.app_handle())?;

            debug!("Starting setup of RQuickShare-X app");

            // Initialize default values for the store
            init_default(app.app_handle());

            // Fetch initial configuration values
            let visibility = get_visibility(app.app_handle());
            let port_number = get_port(app.app_handle());
            let download_path = get_download_path(app.app_handle());

            // Initialize system Tray
            let name = MenuItemBuilder::new("RQuickShare-X")
                .enabled(false)
                .build(app)?;
            let show = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let visible = CheckMenuItemBuilder::with_id("visible", "Visible")
                .checked(visibility == Visibility::Visible)
                .build(app)?;
            let invisible = CheckMenuItemBuilder::with_id("invisible", "Invisible")
                .checked(visibility == Visibility::Invisible)
                .build(app)?;
            let temporarily = CheckMenuItemBuilder::with_id(
                "temporarily",
                "Temporarily (1 min)",
            )
            .checked(visibility == Visibility::Temporarily)
            .build(app)?;
            let visibility_menu = Arc::new(
                SubmenuBuilder::new(app, "Visibility")
                    .items(&[&visible, &invisible, &temporarily])
                    .build()?,
            );
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&name)
                .separator()
                .items(&[&show, visibility_menu.as_ref(), &quit])
                .build()?;
            let visibility_menu_for_handler = Arc::clone(&visibility_menu);

            #[cfg(target_os = "macos")]
            let icon = Image::from_bytes(include_bytes!("../icons/tray.png")).unwrap();
            #[cfg(not(target_os = "macos"))]
            let icon = app.default_window_icon().unwrap().clone();

            let tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        trace!("tray_show");
                        open_main_window(app);
                    }
                    "visible" | "invisible" | "temporarily" => {
                        let visibility = match event.id().as_ref() {
                            "visible" => Visibility::Visible,
                            "invisible" => Visibility::Invisible,
                            "temporarily" => Visibility::Temporarily,
                            _ => unreachable!(),
                        };

                        app.state::<AppState>()
                            .rqs
                            .lock()
                            .unwrap()
                            .change_visibility(visibility);
                        visibility_menu_for_handler
                            .get("visible")
                            .unwrap()
                            .as_check_menuitem_unchecked()
                            .set_checked(visibility == Visibility::Visible)
                            .unwrap();
                        visibility_menu_for_handler
                            .get("invisible")
                            .unwrap()
                            .as_check_menuitem_unchecked()
                            .set_checked(visibility == Visibility::Invisible)
                            .unwrap();
                        visibility_menu_for_handler
                            .get("temporarily")
                            .unwrap()
                            .as_check_menuitem_unchecked()
                            .set_checked(visibility == Visibility::Temporarily)
                            .unwrap();
                    }
                    "quit" => {
                        trace!("tray_quit");
                        kill_app(app.app_handle());
                    }
                    _ => (),
                })
                .build(app)?;

            let _ = tray.set_icon_as_template(true);

            let app_handle = app.app_handle().clone();
            // This is not optimal, but until I find a better way to init log
            // (inside file and stdout) before starting the lib, I'll keep it as
            // is. This allow me to get the whole log :)
            tokio::task::block_in_place(|| {
                tauri::async_runtime::block_on(async move {
                    trace!("Beginning of RQS start");
                    // Start the RQuickShare-X service
                    let mut rqs = RQS::new(visibility, port_number, download_path);
                    let (sender_file, ble_receiver) = rqs.run().await.unwrap();

                    // Define state for tauri app
                    app_handle.manage(AppState {
                        message_sender: rqs.message_sender.clone(),
                        dch_sender: broadcast::channel(10).0,
                        visibility_sender: rqs.visibility_sender.clone(),
                        sender_file,
                        ble_receiver,
                        rqs: Mutex::new(rqs),
                        pending_files: Mutex::new(
                            std::env::args()
                                .skip(1)
                                .map(|arg| arg.trim_matches('"').to_string())
                                .filter(|arg| !arg.starts_with('-') && arg != "%1")
                                .collect(),
                        ),
                    });
                });
            });

            spawn_receiver_tasks(app.app_handle());
            Ok(())
        })
        .on_window_event(handle_window_event)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready { .. } => {
                trace!("RunEvent::Ready");
                if get_startminimized(app_handle) {
                    #[cfg(not(target_os = "macos"))]
                    app_handle
                        .get_webview_window("main")
                        .unwrap()
                        .hide()
                        .unwrap();
                    #[cfg(target_os = "macos")]
                    app_handle.hide().unwrap();
                }
            }
            tauri::RunEvent::ExitRequested { code, .. } => {
                trace!("RunEvent::ExitRequested");
                if code != Some(-1) {
                    kill_app(app_handle);
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                trace!("RunEvent::Reopen");
                open_main_window(app_handle);
            }
            _ => {}
        });

    info!("Application stopped");
    Ok(())
}

#[tauri::command]
fn take_pending_files(state: tauri::State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *state.pending_files.lock().unwrap())
}

fn spawn_receiver_tasks(app_handle: &AppHandle) {
    let capp_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state: tauri::State<'_, AppState> = capp_handle.state();
        let mut receiver = state.message_sender.subscribe();

        loop {
            let rinfo = receiver.recv().await;

            match rinfo {
                Ok(info) => {
                    if info.state.as_ref().unwrap_or(&State::Initial)
                        == &State::WaitingForUserConsent
                    {
                        let name = info
                            .meta
                            .as_ref()
                            .and_then(|meta| meta.source.as_ref())
                            .map(|source| source.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        send_request_notification(name, info.id.clone(), &capp_handle);
                    }
                    rs2js_channelmessage(info, &capp_handle);
                }
                Err(e) => {
                    error!("RecvError: message_sender: {e}");
                }
            }
        }
    });

    let capp_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state: tauri::State<'_, AppState> = capp_handle.state();
        let mut dch_receiver = state.dch_sender.subscribe();

        loop {
            let rinfo = dch_receiver.recv().await;

            match rinfo {
                Ok(info) => rs2js_endpointinfo(info, &capp_handle),
                Err(e) => {
                    error!("RecvError: dch_sender: {e}");
                }
            }
        }
    });

    let capp_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state: tauri::State<'_, AppState> = capp_handle.state();
        let mut visibility_receiver = state.visibility_sender.lock().unwrap().subscribe();

        loop {
            let rinfo = visibility_receiver.changed().await;

            match rinfo {
                Ok(_) => {
                    let v = visibility_receiver.borrow_and_update();
                    let _ = set_visibility(&capp_handle, *v);
                }
                Err(e) => {
                    error!("RecvError: visibility_receiver: {e}");
                }
            }
        }
    });

    let capp_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let state: tauri::State<'_, AppState> = capp_handle.state();
        let mut ble_receiver = state.ble_receiver.resubscribe();
        let mut last_sent = std::time::Instant::now() - std::time::Duration::from_secs(120);

        loop {
            let rinfo = ble_receiver.recv().await;

            match rinfo {
                Ok(_) => {
                    let v = get_visibility(&capp_handle);
                    trace!("Tauri: ble received: {:?}", v);

                    if v == Visibility::Invisible
                        && last_sent.elapsed() >= std::time::Duration::from_secs(120)
                    {
                        send_temporarily_notification(&capp_handle);
                        last_sent = std::time::Instant::now();
                    }
                }
                Err(e) => {
                    error!("RecvError: ble_receiver: {e}");
                }
            }
        }
    });
}

fn handle_window_event(w: &Window, event: &WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if get_realclose(w.app_handle()) {
            trace!("handle_window_event: real close");
            return;
        }

        trace!("handle_window_event: prevent close");
        w.hide().unwrap();
        api.prevent_close();
    }
}

fn rs2js_channelmessage(message: ChannelMessage, manager: &AppHandle) {
    if message.direction == ChannelDirection::FrontToLib {
        return;
    }

    info!("rs2js_channelmessage: {:?}", &message);
    manager.emit("rs2js_channelmessage", &message).unwrap();
}

fn rs2js_endpointinfo(message: EndpointInfo, manager: &AppHandle) {
    info!("rs2js_endpointinfo: {:?}", &message);
    manager.emit("rs2js_endpointinfo", &message).unwrap();
}

fn open_main_window(app_handle: &AppHandle) {
    if let Some(webview_window) = app_handle.get_webview_window("main") {
        let _ = webview_window.show();
        let _ = webview_window.set_focus();
        return;
    }

    warn!("open_main_window: no main window found");
}

fn kill_app(app_handle: &AppHandle) {
    let state: tauri::State<'_, AppState> = app_handle.state();

    tokio::task::block_in_place(|| {
        #[allow(clippy::await_holding_lock)]
        tauri::async_runtime::block_on(async move {
            let _ = state.rqs.lock().unwrap().stop().await;
        });
    });

    app_handle.exit(-1);
}
