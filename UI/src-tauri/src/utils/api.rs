use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use crate::{utils::custom_result::CustomResult, AppState, OpenCVResource};
use opencv::{
    core::{Mat, Size},
    objdetect::{FaceDetectorYN, FaceRecognizerSF}, videoio::{VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};
use serde_json::json;
use tauri::Manager;
use windows::{
    core::{HRESULT, HSTRING, PWSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileW, WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE, OPEN_EXISTING,
        },
        System::{
            Pipes::WaitNamedPipeW, Shutdown::LockWorkStation, WindowsProgramming::GetUserNameW,
        },
    },
};

// 获取当前用户名
#[tauri::command]
pub fn get_now_username() -> Result<CustomResult, CustomResult> {
    // buffer大小，256应该够了
    let mut buffer = [0u16; 256];
    let mut size = buffer.len() as u32;
    unsafe {
        let succuess = GetUserNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size);
        if succuess.is_err() {
            return Err(CustomResult::error(
                Some(format!("获取用户名失败: {:?}", succuess.err())),
                None,
            ));
        }

        let name = String::from_utf16_lossy(&buffer[..size as usize - 1]);
        return Ok(CustomResult::success(None, Some(json!({"username": name}))));
    }
}

// 测试 WinLogon 是否加载成功
#[tauri::command]
pub fn test_win_logon(user_name: String, password: String) -> Result<CustomResult, CustomResult> {
    // 锁定屏幕
    unsafe {
        let succuess = LockWorkStation();
        if succuess.is_err() {
            return Err(CustomResult::error(
                Some(format!("锁定屏幕失败: {:?}", succuess.err())),
                None,
            ));
        }

        // 等待5秒
        std::thread::sleep(std::time::Duration::from_secs(5));
        // 解锁
        unlock(user_name, password)
            .map_err(|e| CustomResult::error(Some(format!("解锁屏幕失败: {:?}", e)), None))?;
    }
    return Ok(CustomResult::success(None, None));
}

// 初始化模型
#[tauri::command]
pub async fn init_model(
    handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CustomResult, CustomResult> {
    let resource_path = handle
        .path()
        .resolve(
            format!("resources/{}", "face_detection_yunet_2023mar.onnx"),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| CustomResult::error(Some(format!("路径解析失败: {}", e)), None))?;

    // 这个不用检查文件是否存在，不存在opencv会报错
    let detector = FaceDetectorYN::create(
        resource_path.to_str().unwrap_or(""),
        "",
        Size::new(320, 320), // 初始尺寸，后面会动态更新
        0.9,
        0.3,
        5000,
        0,
        0,
    )
    .map_err(|e| CustomResult::error(Some(format!("初始化检测器模型失败: {:?}", e)), None))?;

    let resource_path = handle
        .path()
        .resolve(
            format!("resources/{}", "face_recognition_sface_2021dec.onnx"),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| CustomResult::error(Some(format!("路径解析失败: {}", e)), None))?;
    let recognizer = FaceRecognizerSF::create(resource_path.to_str().unwrap_or(""), "", 0, 0)
        .map_err(|e| CustomResult::error(Some(format!("初始化识别器模型失败: {:?}", e)), None))?;

    let mut d_lock = state.detector.write().await;
    // 包装进 OpenCVResource
    *d_lock = Some(OpenCVResource { inner: detector });

    let mut r_lock = state.recognizer.write().await;
    *r_lock = Some(OpenCVResource { inner: recognizer });

    Ok(CustomResult::success(None, None))
}

// 打开摄像头
#[tauri::command]
pub async fn open_camera(state: tauri::State<'_, AppState>) -> Result<CustomResult, CustomResult> {

    let mut cam_lock = state.camera.write().await;
    // 如果摄像头没打开
    if cam_lock.is_none() {
        let mut cam = VideoCapture::new(0, opencv::videoio::CAP_ANY)
            .map_err(|e| CustomResult::error(Some(format!("摄像头打开失败: {}", e)), None))?;

        let is_opened = cam.is_opened().map_err(|e| CustomResult::error(Some(format!("检查状态失败: {}", e)), None))?;
        if !is_opened {
            return Err(CustomResult::error(Some("摄像头打开失败，设备可能被占用".to_string()), None));
        }

        // 读取一帧 激活摄像头
        let mut frame = Mat::default();
        cam.read(&mut frame).map_err(|e| CustomResult::error(Some(format!("激活失败: {}", e)), None))?;

        *cam_lock = Some(OpenCVResource { inner: cam });
    }

    Ok(CustomResult::success(None, None))
}

// 关闭摄像头
#[tauri::command]
pub async fn stop_camera(state: tauri::State<'_, AppState>) -> Result<CustomResult, ()> {
    let mut cam_lock = state.camera.write().await;
    *cam_lock = None;
    Ok(CustomResult::success(None, None))
}

// 解锁屏幕
pub fn unlock(user_name: String, password: String) -> windows::core::Result<()> {
    unsafe {
        let pipe_name = HSTRING::from("\\\\.\\pipe\\MansonWindowsUnlockRust");
        // 等待管道连接
        if !WaitNamedPipeW(&pipe_name.clone(), 5000).as_bool() {
            return Err(windows::core::Error::new(
                HRESULT(0),
                "不能连接到管道: MansonWindowsUnlockRust",
            ));
        }

        // 打开管道
        let handle = CreateFileW(
            &pipe_name.clone(), // 管道名称
            GENERIC_WRITE.0,    // 对文件的操作模式，只写
            FILE_SHARE_MODE(0), // 阻止对管道的后续打开操作，在我主动关闭之前
            None,
            OPEN_EXISTING, // 只在文件存在时才打开，否则返回错误
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        );
        if handle.is_err() {
            return Err(windows::core::Error::new(
                HRESULT(0),
                format!("打开管道失败: {:?}", handle.err()),
            ));
        }
        let handle = handle.unwrap();

        // 向管道发送用户名
        let write_success = send_to_pipe(user_name, handle);
        if write_success.is_err() {
            let _ = CloseHandle(handle);
            return Err(windows::core::Error::new(
                HRESULT(0),
                format!(
                    "发送用户名失败: {:?}, 扩展信息: {:?}",
                    write_success.err(),
                    GetLastError()
                ),
            ));
        }

        // 向管道发送密码
        let write_success = send_to_pipe(password, handle);
        if write_success.is_err() {
            let _ = CloseHandle(handle);
            return Err(windows::core::Error::new(
                HRESULT(0),
                format!(
                    "发送密码失败: {:?}, 扩展信息: {:?}",
                    write_success.err(),
                    GetLastError()
                ),
            ));
        }

        let _ = CloseHandle(handle);
    };

    Ok(())
}

// 向管道发送数据
fn send_to_pipe(content: String, handle: HANDLE) -> windows::core::Result<()> {
    unsafe {
        // 转 UTF-16 含 \0
        let wide_chars: Vec<u16> = OsStr::new(&content)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // 转 &[u8] 切片
        let write_buf =
            std::slice::from_raw_parts(wide_chars.as_ptr() as *const u8, wide_chars.len() * 2);
        // 准备字节数
        let mut total_bytes = write_buf.len() as u32;

        WriteFile(handle, Some(write_buf), Some(&mut total_bytes), None)
    }
}
