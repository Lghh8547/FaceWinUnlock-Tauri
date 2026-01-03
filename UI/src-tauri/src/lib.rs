// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::{async_runtime::RwLock, Manager};
use windows::Win32::{
    Foundation::HWND,
    System::RemoteDesktop::{
        WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
    },
    UI::Shell::SetWindowSubclass,
};

pub mod modules;
pub mod proc;
pub mod utils;
use modules::faces::{check_face_from_img, check_face_from_camera, verify_face, save_face_registration};
use modules::init::{check_admin_privileges, check_camera_status, deploy_core_components};
use opencv::{
    core::Ptr,
    objdetect::{FaceDetectorYN, FaceRecognizerSF}, videoio::VideoCapture,
};
use proc::wnd_proc_subclass;
use utils::api::{get_now_username, init_model, test_win_logon, open_camera, stop_camera};

pub struct OpenCVResource<T> {
    pub inner: T,
}
unsafe impl<T> Send for OpenCVResource<T> {}
unsafe impl<T> Sync for OpenCVResource<T> {}
// 持久存储模型
pub struct AppState {
    pub detector: RwLock<Option<OpenCVResource<Ptr<FaceDetectorYN>>>>,
    pub recognizer: RwLock<Option<OpenCVResource<Ptr<FaceRecognizerSF>>>>,
    pub camera: RwLock<Option<OpenCVResource<VideoCapture>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            detector: RwLock::new(None),
            recognizer: RwLock::new(None),
            camera: RwLock::new(None)
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            #[cfg(debug_assertions)] // 仅在调试(debug)版本中包含此代码
            {
                window.open_devtools();
                window.close_devtools();
            }

            #[cfg(windows)]
            {
                let window = app.get_webview_window("main").unwrap();
                let hwnd = window.hwnd().unwrap();
                unsafe {
                    // 注册 WTS 通知
                    let _ = WTSRegisterSessionNotification(HWND(hwnd.0), NOTIFY_FOR_THIS_SESSION);

                    // 注入子类化回调来捕获 WM_WTSSESSION_CHANGE
                    // on_window_event 收不到这个消息
                    let _ = SetWindowSubclass(HWND(hwnd.0), Some(wnd_proc_subclass), 0, 0);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                match event {
                    tauri::WindowEvent::CloseRequested { .. } => {
                        let hwnd = window.hwnd().unwrap();
                        unsafe {
                            // 注销 WTS 通知
                            let _ = WTSUnRegisterSessionNotification(HWND(hwnd.0));
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // init 初始化模块
            check_admin_privileges,
            check_camera_status,
            deploy_core_components,
            // 面容模块
            check_face_from_img,
            check_face_from_camera,
            verify_face,
            save_face_registration,
            // 通用api
            get_now_username,
            test_win_logon,
            init_model,
            open_camera,
            stop_camera,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
