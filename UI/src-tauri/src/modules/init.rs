use crate::utils::custom_result::CustomResult;
use opencv::videoio::{self, VideoCaptureTraitConst};
use std::fs;
use tauri::Manager;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};
use winreg::enums::*;
use winreg::RegKey;

// 检查是否具有管理员权限
#[tauri::command]
pub fn check_admin_privileges() -> Result<CustomResult, CustomResult> {
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        let success = OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token);
        if let Ok(_) = success {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;

            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                size,
                &mut size,
            );

            let _ = CloseHandle(token);
            if result.is_ok() && elevation.TokenIsElevated != 0 {
                return Ok(CustomResult::success(None, None));
            }
        } else {
            return Err(CustomResult::error(
                Some(format!("打开与进程关联的访问令牌失败：{:?}", success.err())),
                None,
            ));
        }
    }
    return Err(CustomResult::error(
        Some(String::from("无管理员权限，请右键 ‘以管理员身份’ 运行软件")),
        None,
    ));
}

// 检查摄像头是否可用
#[tauri::command]
pub fn check_camera_status() -> Result<CustomResult, CustomResult> {
    let mut available_cameras = Vec::new();

    // 0 通常表示默认摄像头
    // 1 通常表示外置摄像头
    // 循环判断
    for i in 0..2 {
        if let Ok(cam) = videoio::VideoCapture::new(i, videoio::CAP_ANY) {
            if videoio::VideoCapture::is_opened(&cam).unwrap_or(false) {
                available_cameras.push(format!("摄像头索引 {}", i));
            }
        }
    }

    if available_cameras.is_empty() {
        Err(CustomResult::error(
            Some(String::from("未找到可用摄像头或权限被拒绝")),
            None,
        ))
    } else {
        Ok(CustomResult::success(None, None))
    }
}

// 复制 DLL 并写入注册表
#[tauri::command]
pub fn deploy_core_components(handle: tauri::AppHandle) -> Result<CustomResult, CustomResult> {
    let dll_name = "FaceWinUnlock-Tauri.dll";
    let target_path = format!("C:\\Windows\\System32\\{}", dll_name);

    // 获取 resources 中的 DLL 路径
    let resource_path = handle
        .path()
        .resolve(
            format!("resources/{}", dll_name),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| CustomResult::error(Some(format!("路径解析失败: {}", e)), None))?;

    // 检查资源文件是否存在
    if !resource_path.exists() {
        return Err(CustomResult::error(
            Some(format!("资源文件不存在: {:?}", resource_path.to_str())),
            None,
        ));
    }

    // 复制文件到 System32
    fs::copy(&resource_path, &target_path).map_err(|e| {
        CustomResult::error(
            Some(format!(
                "DLL 复制失败: {} 请确认是否以管理员身份运行，或文件是否被占用",
                e
            )),
            None,
        )
    })?;

    // 写入注册表
    let clsid = "{8a7b9c6d-4e5f-89a0-8b7c-6d5e4f3e2d1c}";

    // 使用 KEY_ALL_ACCESS 确保拥有写入权限
    let hk_lm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hk_cr = RegKey::predef(HKEY_CLASSES_ROOT);

    // 注册 Credential Provider
    let cp_path = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Authentication\\Credential Providers\\{}",
        clsid
    );
    let (cp_key, _) = hk_lm
        .create_subkey(cp_path)
        .map_err(|e| CustomResult::error(Some(format!("无法创建注册表项(CP): {}", e)), None))?;
    cp_key
        .set_value("", &"FaceWinUnlock-Tauri")
        .map_err(|e| CustomResult::error(Some(format!("无法设置注册表项(CP): {}", e)), None))?;

    // 注册 CLSID
    let clsid_path = format!("CLSID\\{}", clsid);
    let (clsid_key, _) = hk_cr
        .create_subkey(&clsid_path)
        .map_err(|e| CustomResult::error(Some(format!("无法创建注册表项(CLSID): {}", e)), None))?;
    clsid_key
        .set_value("", &"FaceWinUnlock-Tauri")
        .map_err(|e| CustomResult::error(Some(format!("无法设置注册表项(CLSID): {}", e)), None))?;

    let (inproc_key, _) = hk_cr
        .create_subkey(format!("{}\\InprocServer32", clsid_path))
        .map_err(|e| {
            CustomResult::error(
                Some(format!("无法创建注册表项(InprocServer32): {}", e)),
                None,
            )
        })?;

    inproc_key.set_value("", &target_path).map_err(|e| {
        CustomResult::error(
            Some(format!("无法设置注册表项(InprocServer32): {}", e)),
            None,
        )
    })?;
    inproc_key
        .set_value("ThreadingModel", &"Apartment")
        .map_err(|e| {
            CustomResult::error(
                Some(format!("无法设置注册表项(ThreadingModel): {}", e)),
                None,
            )
        })?;

    Ok(CustomResult::success(None, None))
}
