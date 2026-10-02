use crate::domain::{ClipboardItem, ItemType};
use chrono::Local;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable,
    OpenClipboard, RemoveClipboardFormatListener,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::*;

const CF_TEXT: u32 = 1;
const CF_BITMAP: u32 = 2;
const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP_ID: u32 = 15;

#[allow(dead_code)]
pub struct ClipboardListener {
    running: Arc<AtomicBool>,
}

impl ClipboardListener {
    pub fn start(tx: Sender<ClipboardItem>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        std::thread::spawn(move || {
            run_listener_loop(tx, running_clone);
        });

        Self { running }
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn run_listener_loop(tx: Sender<ClipboardItem>, running: Arc<AtomicBool>) {
    unsafe {
        let class_name = windows::core::w!("ClipboardShelfListenerClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let hwnd_res = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            windows::core::w!("ClipboardShelfListenerWindow"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            HMODULE::default(),
            None,
        );

        let hwnd = match hwnd_res {
            Ok(h) => h,
            Err(e) => {
                eprintln!("Failed to create clipboard listener window: {:?}", e);
                return;
            }
        };

        let add_res = AddClipboardFormatListener(hwnd);
        println!("--> [Clipboard Thread] AddClipboardFormatListener returned: {:?}", add_res);

        let mut last_content: Option<String> = None;

        // Startup: capture whatever is currently on the clipboard so shelf starts with user's last copy!
        if let Some(initial_item) = read_clipboard_item() {
            println!("--> [Clipboard Thread] Initial capture: '{}'", initial_item.title);
            if let Some(t) = &initial_item.text_content {
                last_content = Some(t.clone());
            } else if let Some(f) = initial_item.file_paths.first() {
                last_content = Some(f.clone());
            }
            let _ = tx.send(initial_item);
        }

        let mut msg = MSG::default();

        while running.load(Ordering::Relaxed) {
            let has_msg = PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE);
            if has_msg.as_bool() {
                if msg.message == WM_CLIPBOARDUPDATE {
                    println!("--> [Clipboard Thread] WM_CLIPBOARDUPDATE received!");
                    if let Some(item) = read_clipboard_item() {
                        let is_duplicate = match (&last_content, &item.text_content, item.file_paths.first()) {
                            (Some(last), Some(current), _) => last == current,
                            (Some(last), None, Some(first_file)) => last == first_file,
                            _ => false,
                        };

                        if !is_duplicate {
                            if let Some(t) = &item.text_content {
                                last_content = Some(t.clone());
                            } else if let Some(f) = item.file_paths.first() {
                                last_content = Some(f.clone());
                            }
                            println!("--> [Clipboard Thread] Dispatching new item: '{}'", item.title);
                            let _ = tx.send(item);
                        } else {
                            println!("--> [Clipboard Thread] Skipped duplicate item: '{}'", item.title);
                        }
                    }
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn read_clipboard_item() -> Option<ClipboardItem> {
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(HWND::default()).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        if !opened {
            return None;
        }

        let mut result = None;

        // 1. Files (CF_HDROP)
        if IsClipboardFormatAvailable(CF_HDROP_ID).is_ok() {
            if let Ok(handle) = GetClipboardData(CF_HDROP_ID) {
                let hdrop = HDROP(handle.0);
                let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
                let mut paths = Vec::new();

                for i in 0..count {
                    let len = DragQueryFileW(hdrop, i, None);
                    if len > 0 {
                        let mut buffer = vec![0u16; (len + 1) as usize];
                        DragQueryFileW(hdrop, i, Some(&mut buffer));
                        let path_str = String::from_utf16_lossy(&buffer[..len as usize]);
                        paths.push(path_str);
                    }
                }

                if !paths.is_empty() {
                    result = Some(ClipboardItem::new_files(paths));
                }
            }
        }

        // 2. Unicode Text (CF_UNICODETEXT)
        if result.is_none() && IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() {
            if let Ok(handle) = GetClipboardData(CF_UNICODETEXT) {
                let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
                if !ptr.is_null() {
                    let wide_slice = std::slice::from_raw_parts(
                        ptr as *const u16,
                        wcslen(ptr as *const u16),
                    );
                    let text = String::from_utf16_lossy(wide_slice);
                    let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));

                    if !text.is_empty() {
                        result = Some(ClipboardItem::new_text(text));
                    }
                }
            }
        }

        // 3. Fallback ANSI Text (CF_TEXT)
        if result.is_none() && IsClipboardFormatAvailable(CF_TEXT).is_ok() {
            if let Ok(handle) = GetClipboardData(CF_TEXT) {
                let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
                if !ptr.is_null() {
                    let c_str = std::ffi::CStr::from_ptr(ptr as *const i8);
                    let text = c_str.to_string_lossy().to_string();
                    let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));

                    if !text.is_empty() {
                        result = Some(ClipboardItem::new_text(text));
                    }
                }
            }
        }

        // 4. Image data (CF_DIB / CF_BITMAP)
        if result.is_none() && (IsClipboardFormatAvailable(CF_DIB).is_ok() || IsClipboardFormatAvailable(CF_BITMAP).is_ok()) {
            let mut file_path = None;
            if let Ok(handle) = GetClipboardData(CF_DIB) {
                let hglobal = windows::Win32::Foundation::HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let size = windows::Win32::System::Memory::GlobalSize(hglobal);
                    if size >= 40 {
                        let dib_bytes = std::slice::from_raw_parts(ptr as *const u8, size);
                        let temp_dir = std::env::temp_dir().join("clipboard-shelf");
                        let _ = std::fs::create_dir_all(&temp_dir);
                        let file_name = format!("image_{}.bmp", Local::now().format("%Y%m%d_%H%M%S"));
                        let full_path = temp_dir.join(file_name);

                        let bi_size = u32::from_le_bytes([dib_bytes[0], dib_bytes[1], dib_bytes[2], dib_bytes[3]]);
                        let bi_bit_count = u16::from_le_bytes([dib_bytes[14], dib_bytes[15]]);
                        let bi_clr_used = u32::from_le_bytes([dib_bytes[32], dib_bytes[33], dib_bytes[34], dib_bytes[35]]);

                        let palette_colors = if bi_clr_used > 0 {
                            bi_clr_used as usize
                        } else if bi_bit_count <= 8 {
                            1usize << bi_bit_count
                        } else {
                            0
                        };
                        let offset_to_bits = 14 + bi_size as usize + (palette_colors * 4);
                        let file_size = 14 + size;

                        let mut bmp_data = Vec::with_capacity(file_size);
                        bmp_data.extend_from_slice(b"BM");
                        bmp_data.extend_from_slice(&(file_size as u32).to_le_bytes());
                        bmp_data.extend_from_slice(&0u32.to_le_bytes());
                        bmp_data.extend_from_slice(&(offset_to_bits as u32).to_le_bytes());
                        bmp_data.extend_from_slice(dib_bytes);

                        if std::fs::write(&full_path, bmp_data).is_ok() {
                            file_path = Some(full_path.to_string_lossy().to_string());
                        }
                    }
                    let _ = GlobalUnlock(hglobal);
                }
            }

            let paths = if let Some(p) = file_path { vec![p] } else { Vec::new() };
            result = Some(ClipboardItem {
                id: uuid::Uuid::new_v4().to_string(),
                title: format!("Captured Image {}", Local::now().format("%H:%M:%S")),
                preview: "Screenshot / Copied Image".to_string(),
                item_type: ItemType::Image,
                text_content: None,
                file_paths: paths,
                image_data: None,
                pinned: false,
                captured_at: Local::now(),
            });
        }

        let _ = CloseClipboard();
        result
    }
}

unsafe fn wcslen(ptr: *const u16) -> usize {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    len
}
