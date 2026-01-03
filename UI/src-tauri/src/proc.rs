use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK},
    },
};

// windows回调
pub unsafe extern "system" fn wnd_proc_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        let event_type = wparam.0 as u32;
        let session_id = lparam.0 as u32;

        match event_type {
            WTS_SESSION_LOCK => {
                println!("[会话{}] 屏幕已锁屏", session_id);
            }
            WTS_SESSION_UNLOCK => {
                println!("[会话{}] 屏幕已解锁", session_id);
            }
            _ => {}
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
