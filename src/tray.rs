use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAYICON: u32 = 0x8000 + 101;
pub const TRAY_CMD_SHOW_HIDE: usize = 2001;
pub const TRAY_CMD_AUTO_HIDE: usize = 2002;
pub const TRAY_CMD_CLEAR_ALL: usize = 2003;
pub const TRAY_CMD_TOGGLE_LANG: usize = 2004;
pub const TRAY_CMD_EXIT: usize = 2009;

pub static TRAY_CLEAR_TRIGGERED: AtomicBool = AtomicBool::new(false);

pub fn attach_to_default_desktop() {
    unsafe {
        let user32 = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("user32.dll")).unwrap_or_default();
        if !user32.is_invalid() {
            if let Some(open_desktop) = windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("OpenDesktopW")) {
                let open_desktop_fn: extern "system" fn(*const u16, u32, i32, u32) -> usize = std::mem::transmute(open_desktop);
                if let Some(set_desktop) = windows::Win32::System::LibraryLoader::GetProcAddress(user32, windows::core::s!("SetThreadDesktop")) {
                    let set_desktop_fn: extern "system" fn(usize) -> i32 = std::mem::transmute(set_desktop);
                    let default_name: Vec<u16> = "Default\0".encode_utf16().collect();
                    let h_def = open_desktop_fn(default_name.as_ptr(), 0, 0, 0x1FF);
                    if h_def != 0 {
                        let _ = set_desktop_fn(h_def);
                    }
                }
            }
        }
    }
}

pub fn add_tray_icon(hwnd: HWND) -> bool {
    attach_to_default_desktop();
    unsafe {
        let mut hicon = HICON::default();
        if let Ok(h) = LoadImageW(
            None,
            windows::core::w!("assets\\app_icon.ico"),
            IMAGE_ICON,
            16,
            16,
            LR_LOADFROMFILE | LR_DEFAULTCOLOR,
        ) {
            if !h.is_invalid() {
                hicon = HICON(h.0);
            }
        }
        if hicon.is_invalid() {
            if let Ok(hinst) = windows::Win32::System::LibraryLoader::GetModuleHandleW(None) {
                let hinst_inst = windows::Win32::Foundation::HINSTANCE(hinst.0);
                if let Ok(h) = LoadImageW(
                    hinst_inst,
                    windows::core::PCWSTR(1 as *const u16),
                    IMAGE_ICON,
                    16,
                    16,
                    LR_DEFAULTCOLOR,
                ) {
                    if !h.is_invalid() {
                        hicon = HICON(h.0);
                    }
                }
            }
        }
        if hicon.is_invalid() {
            hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1001,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: hicon,
            szTip: [0; 128],
            ..Default::default()
        };

        let tip = "Clipboard Shelf";
        for (i, c) in tip.encode_utf16().enumerate().take(127) {
            nid.szTip[i] = c;
        }

        let res = Shell_NotifyIconW(NIM_ADD, &nid).as_bool();
        println!("--> [Tray] Shell_NotifyIconW NIM_ADD returned: {}", res);
        res
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    attach_to_default_desktop();
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1001,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        println!("--> [Tray] Shell_NotifyIconW NIM_DELETE executed");
    }
}

pub unsafe fn handle_tray_message(hwnd: HWND, lparam: LPARAM) {
    let event = (lparam.0 & 0xffff) as u32;

    match event {
        WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
            crate::bring_shelf_to_front();
        }
        WM_RBUTTONUP | WM_CONTEXTMENU => {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            if let Ok(hmenu) = CreatePopupMenu() {
                let auto_hide_active = crate::is_auto_hide_enabled();
                let check_flag = if auto_hide_active { MF_CHECKED } else { MF_UNCHECKED };
                let is_en = crate::get_locale() == "en";

                let show_hide_txt = if is_en {
                    windows::core::w!("📋 Show / Hide Clipboard Shelf")
                } else {
                    windows::core::w!("📋 Hiện / Ẩn Clipboard Shelf")
                };

                let auto_hide_txt = if is_en {
                    windows::core::w!("Auto-hide to screen edge")
                } else {
                    windows::core::w!("Tự động ẩn vào cạnh màn hình")
                };

                let clear_all_txt = if is_en {
                    windows::core::w!("🧹 Clear unpinned cards")
                } else {
                    windows::core::w!("🧹 Xoá các thẻ chưa ghim")
                };

                let lang_txt = if is_en {
                    windows::core::w!("🌐 Switch to Vietnamese")
                } else {
                    windows::core::w!("🌐 Chuyển sang Tiếng Anh")
                };

                let exit_txt = if is_en {
                    windows::core::w!("❌ Exit Clipboard Shelf")
                } else {
                    windows::core::w!("❌ Thoát Clipboard Shelf")
                };

                let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_SHOW_HIDE, show_hide_txt);
                let _ = AppendMenuW(hmenu, MF_STRING | check_flag, TRAY_CMD_AUTO_HIDE, auto_hide_txt);
                let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_CLEAR_ALL, clear_all_txt);
                let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_TOGGLE_LANG, lang_txt);
                let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(hmenu, MF_STRING, TRAY_CMD_EXIT, exit_txt);

                // Set foreground window to hwnd so menu automatically dismisses when clicking elsewhere
                let _ = SetForegroundWindow(hwnd);

                let cmd = TrackPopupMenu(
                    hmenu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_BOTTOMALIGN,
                    pt.x,
                    pt.y,
                    0,
                    hwnd,
                    None,
                );
                let _ = DestroyMenu(hmenu);

                match cmd.0 as usize {
                    TRAY_CMD_SHOW_HIDE => {
                        crate::toggle_shelf_visibility();
                    }
                    TRAY_CMD_AUTO_HIDE => {
                        crate::toggle_auto_hide_mode();
                    }
                    TRAY_CMD_CLEAR_ALL => {
                        TRAY_CLEAR_TRIGGERED.store(true, Ordering::SeqCst);
                    }
                    TRAY_CMD_TOGGLE_LANG => {
                        crate::toggle_locale();
                    }
                    TRAY_CMD_EXIT => {
                        crate::cleanup_and_exit(hwnd);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
