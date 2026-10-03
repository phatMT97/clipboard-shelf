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

static IS_INTERNAL_COPY: AtomicBool = AtomicBool::new(false);

pub fn set_internal_copy() {
    IS_INTERNAL_COPY.store(true, Ordering::SeqCst);
}

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
    crate::tray::attach_to_default_desktop();
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
        crate::tray::add_tray_icon(hwnd);

        let mut last_content: Option<String> = None;
        let mut last_image_capture: Option<(String, std::time::Instant)> = None;

        // Startup: capture whatever is currently on the clipboard so shelf starts with user's last copy!
        if let Some(initial_item) = read_clipboard_item() {
            println!("--> [Clipboard Thread] Initial capture: '{}'", initial_item.title);
            if let Some(t) = &initial_item.text_content {
                last_content = Some(t.clone());
            } else if let Some(f) = initial_item.file_paths.first() {
                last_content = Some(f.clone());
                if initial_item.item_type == ItemType::Image {
                    last_image_capture = Some((f.clone(), std::time::Instant::now()));
                }
            }
            let _ = tx.send(initial_item);
        }

        let mut msg = MSG::default();

        while running.load(Ordering::Relaxed) {
            let has_msg = PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE);
            if has_msg.as_bool() {
                if msg.message == WM_CLIPBOARDUPDATE {
                    println!("--> [Clipboard Thread] WM_CLIPBOARDUPDATE received!");
                    if IS_INTERNAL_COPY.swap(false, Ordering::SeqCst) {
                        println!("--> [Clipboard Thread] Ignored WM_CLIPBOARDUPDATE from internal Shelf copy");
                        if let Some(item) = read_clipboard_item() {
                            if let Some(t) = &item.text_content {
                                last_content = Some(t.clone());
                            } else if let Some(f) = item.file_paths.first() {
                                last_content = Some(f.clone());
                            }
                        }
                    } else if let Some(item) = read_clipboard_item() {
                        let is_duplicate = if item.item_type == ItemType::Image {
                            let current_file = item.file_paths.first().cloned().unwrap_or_default();
                            let is_same_file = last_content.as_ref().map(|l| l == &current_file).unwrap_or(false);
                            let is_recent_same_image = if let Some((last_file, time)) = &last_image_capture {
                                (last_file == &current_file || last_file.ends_with(&current_file))
                                    && time.elapsed() < Duration::from_millis(1500)
                            } else {
                                false
                            };
                            is_same_file || is_recent_same_image
                        } else {
                            match (&last_content, &item.text_content, item.file_paths.first()) {
                                (Some(last), Some(current), _) => last == current,
                                (Some(last), None, Some(first_file)) => last == first_file,
                                _ => false,
                            }
                        };

                        if !is_duplicate {
                            if item.item_type == ItemType::Image {
                                if let Some(f) = item.file_paths.first() {
                                    last_content = Some(f.clone());
                                    last_image_capture = Some((f.clone(), std::time::Instant::now()));
                                }
                            } else if let Some(t) = &item.text_content {
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

        crate::tray::remove_tray_icon(hwnd);
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
    if msg == crate::tray::WM_TRAYICON {
        crate::tray::handle_tray_message(hwnd, lparam);
        return LRESULT(0);
    }
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
            let mut img_title = format!("Ảnh chụp {}", Local::now().format("%H:%M:%S"));
            let mut img_preview = "Ảnh chụp màn hình / Clipboard".to_string();

            if let Ok(handle) = GetClipboardData(CF_DIB) {
                let hglobal = windows::Win32::Foundation::HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal);
                if !ptr.is_null() {
                    let size = windows::Win32::System::Memory::GlobalSize(hglobal);
                    if size >= 40 {
                        let dib_bytes = std::slice::from_raw_parts(ptr as *const u8, size);
                        let temp_dir = std::env::temp_dir().join("clipboard-shelf");
                        let _ = std::fs::create_dir_all(&temp_dir);

                        let bi_size = u32::from_le_bytes([dib_bytes[0], dib_bytes[1], dib_bytes[2], dib_bytes[3]]);
                        let bi_bit_count = u16::from_le_bytes([dib_bytes[14], dib_bytes[15]]);
                        let bi_compression = if size >= 20 {
                            u32::from_le_bytes([dib_bytes[16], dib_bytes[17], dib_bytes[18], dib_bytes[19]])
                        } else {
                            0
                        };
                        let bi_clr_used = u32::from_le_bytes([dib_bytes[32], dib_bytes[33], dib_bytes[34], dib_bytes[35]]);

                        let palette_colors = if bi_clr_used > 0 {
                            bi_clr_used as usize
                        } else if bi_bit_count <= 8 {
                            1usize << bi_bit_count
                        } else {
                            0
                        };
                        let mut offset_to_bits = 14 + bi_size as usize + (palette_colors * 4);
                        if bi_compression == 3 && bi_size == 40 {
                            offset_to_bits += 12; // 3 DWORD color masks (BI_BITFIELDS)
                        }
                        let file_size = 14 + size;

                        let mut bmp_data = Vec::with_capacity(file_size);
                        bmp_data.extend_from_slice(b"BM");
                        bmp_data.extend_from_slice(&(file_size as u32).to_le_bytes());
                        bmp_data.extend_from_slice(&0u32.to_le_bytes());
                        bmp_data.extend_from_slice(&(offset_to_bits as u32).to_le_bytes());
                        bmp_data.extend_from_slice(dib_bytes);

                        let dib_hash = hash_bytes(dib_bytes);
                        let file_name = format!("image_{:016x}.png", dib_hash);
                        let full_path = temp_dir.join(&file_name);

                        if full_path.exists() {
                            file_path = Some(full_path.to_string_lossy().to_string());
                            if dib_bytes.len() >= 12 {
                                let bi_w = i32::from_le_bytes([dib_bytes[4], dib_bytes[5], dib_bytes[6], dib_bytes[7]]).abs() as u32;
                                let bi_h = i32::from_le_bytes([dib_bytes[8], dib_bytes[9], dib_bytes[10], dib_bytes[11]]).abs() as u32;
                                img_title = format!("Ảnh chụp ({}×{})", bi_w, bi_h);
                                img_preview = format!("Ảnh chụp màn hình ({} × {} px)", bi_w, bi_h);
                            }
                        } else if let Ok(img) = image::load_from_memory_with_format(&bmp_data, image::ImageFormat::Bmp) {
                            if img.save_with_format(&full_path, image::ImageFormat::Png).is_ok() {
                                file_path = Some(full_path.to_string_lossy().to_string());
                                img_title = format!("Ảnh chụp ({}×{})", img.width(), img.height());
                                img_preview = format!("Ảnh chụp màn hình ({} × {} px)", img.width(), img.height());
                            }
                        } else {
                            // Fallback if image crate fails to parse BMP: write raw BMP
                            let bmp_path = temp_dir.join(format!("image_{:016x}.bmp", dib_hash));
                            if std::fs::write(&bmp_path, &bmp_data).is_ok() {
                                file_path = Some(bmp_path.to_string_lossy().to_string());
                            }
                        }
                    }
                    let _ = GlobalUnlock(hglobal);
                }
            }

            let paths = if let Some(p) = file_path { vec![p] } else { Vec::new() };
            result = Some(ClipboardItem {
                id: uuid::Uuid::new_v4().to_string(),
                title: img_title,
                preview: img_preview,
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

fn hash_bytes(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let step = (data.len() / 4096).max(1);
    let mut i = 0;
    while i < data.len() {
        h ^= data[i] as u64;
        h = h.wrapping_mul(0x100000001b3);
        i += step;
    }
    h ^ (data.len() as u64)
}

