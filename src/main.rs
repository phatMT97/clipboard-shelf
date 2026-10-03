mod domain;
mod clipboard;
mod transfer;
pub mod tray;
#[cfg(test)]
mod test_cb;
#[cfg(test)]
mod test_clip;
#[cfg(test)]
mod test_transfer;

use domain::{ClipboardItem, ItemType};
use clipboard::ClipboardListener;
use crossbeam_channel::unbounded;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};

slint::include_modules!();

static APP_HWND: AtomicIsize = AtomicIsize::new(0);

pub static AUTO_HIDE_ENABLED: AtomicBool = AtomicBool::new(false);
pub static IS_COLLAPSED: AtomicBool = AtomicBool::new(false);
pub static IS_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

pub static CURRENT_WINDOW_X: AtomicI32 = AtomicI32::new(100);
pub static TARGET_WINDOW_X: AtomicI32 = AtomicI32::new(100);
pub static EXPANDED_WINDOW_X: AtomicI32 = AtomicI32::new(100);
pub static COLLAPSED_WINDOW_X: AtomicI32 = AtomicI32::new(100);
pub static WINDOW_Y: AtomicI32 = AtomicI32::new(100);
pub static WINDOW_W: AtomicI32 = AtomicI32::new(428);
pub static WINDOW_H: AtomicI32 = AtomicI32::new(640);
pub static WORK_AREA_RIGHT: AtomicI32 = AtomicI32::new(1920);
pub static WORK_AREA_TOP: AtomicI32 = AtomicI32::new(0);
pub static WORK_AREA_BOTTOM: AtomicI32 = AtomicI32::new(1080);
pub static LAST_INSIDE_MS: AtomicI64 = AtomicI64::new(0);

static GLOBAL_UI_WEAK: Mutex<Option<slint::Weak<MainWindow>>> = Mutex::new(None);
static CURRENT_LOCALE: Mutex<String> = Mutex::new(String::new());

pub fn get_locale() -> String {
    if let Ok(guard) = CURRENT_LOCALE.lock() {
        if !guard.is_empty() {
            return guard.clone();
        }
    }
    "vi".to_string()
}

pub fn set_locale(loc: &str) {
    if let Ok(mut guard) = CURRENT_LOCALE.lock() {
        *guard = loc.to_string();
    }
    let _ = std::fs::write("C:\\Users\\Admin\\clipboard-shelf\\locale.txt", loc);
}

pub fn toggle_locale() {
    let cur = get_locale();
    let new_loc = if cur == "en" { "vi" } else { "en" };
    set_locale(new_loc);
    log_msg(format!("Language switched: {} -> {}", cur, new_loc));

    if let Ok(guard) = GLOBAL_UI_WEAK.lock() {
        if let Some(weak) = guard.as_ref() {
            let weak_clone = weak.clone();
            let new_loc_str = new_loc.to_string();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak_clone.upgrade() {
                    ui.set_current_locale(new_loc_str.into());
                    ui.invoke_refresh_shelf();
                }
            });
        }
    }
}

fn localize_title_and_preview(title: &str, preview: &str, is_en: bool) -> (String, String) {
    if is_en {
        let t = title
            .replace("Ảnh chụp", "Screenshot")
            .replace("dòng", "lines")
            .replace("Đoạn văn", "Paragraph")
            .replace("Văn bản trống", "Empty text")
            .replace("tệp", "files");
        let p = preview
            .replace("Ảnh chụp màn hình", "Screenshot")
            .replace("Ảnh chụp", "Screenshot");
        (t, p)
    } else {
        let t = title
            .replace("Screenshot", "Ảnh chụp")
            .replace("lines", "dòng")
            .replace("Paragraph", "Đoạn văn")
            .replace("Empty text", "Văn bản trống")
            .replace("files", "tệp");
        let p = preview
            .replace("Screenshot", "Ảnh chụp màn hình");
        (t, p)
    }
}

pub fn is_auto_hide_enabled() -> bool {
    AUTO_HIDE_ENABLED.load(Ordering::SeqCst)
}

pub fn bring_shelf_to_front() {
    IS_WINDOW_VISIBLE.store(true, Ordering::SeqCst);
    let exp_x = EXPANDED_WINDOW_X.load(Ordering::SeqCst);
    TARGET_WINDOW_X.store(exp_x, Ordering::SeqCst);
    IS_COLLAPSED.store(false, Ordering::SeqCst);
    LAST_INSIDE_MS.store(chrono::Local::now().timestamp_millis(), Ordering::SeqCst);

    let win_y = WINDOW_Y.load(Ordering::SeqCst);
    let win_w = WINDOW_W.load(Ordering::SeqCst);
    let win_h = WINDOW_H.load(Ordering::SeqCst);

    if let Some(hwnd) = get_app_hwnd() {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowPos(
                hwnd,
                windows::Win32::Foundation::HWND(-1 as _),
                exp_x,
                win_y,
                win_w,
                win_h,
                SWP_SHOWWINDOW,
            );
            CURRENT_WINDOW_X.store(exp_x, Ordering::SeqCst);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn toggle_shelf_visibility() {
    let cur_vis = IS_WINDOW_VISIBLE.load(Ordering::SeqCst);
    if cur_vis {
        IS_WINDOW_VISIBLE.store(false, Ordering::SeqCst);
        if let Some(hwnd) = get_app_hwnd() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::*;
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    } else {
        bring_shelf_to_front();
    }
}

pub fn toggle_auto_hide_mode() {
    let prev = AUTO_HIDE_ENABLED.fetch_xor(true, Ordering::SeqCst);
    let new_val = !prev;
    log_msg(format!("Auto-hide toggled: {}", new_val));

    if let Ok(guard) = GLOBAL_UI_WEAK.lock() {
        if let Some(weak) = guard.as_ref() {
            let weak_clone = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak_clone.upgrade() {
                    ui.set_auto_hide_enabled(new_val);
                }
            });
        }
    }

    if !new_val {
        let exp_x = EXPANDED_WINDOW_X.load(Ordering::SeqCst);
        TARGET_WINDOW_X.store(exp_x, Ordering::SeqCst);
        IS_COLLAPSED.store(false, Ordering::SeqCst);
    } else {
        LAST_INSIDE_MS.store(chrono::Local::now().timestamp_millis(), Ordering::SeqCst);
    }
}

pub fn cleanup_and_exit(hwnd: windows::Win32::Foundation::HWND) {
    tray::remove_tray_icon(hwnd);
    std::process::exit(0);
}

fn get_app_hwnd() -> Option<windows::Win32::Foundation::HWND> {
    let raw = APP_HWND.load(Ordering::SeqCst);
    if raw != 0 {
        Some(windows::Win32::Foundation::HWND(raw as *mut _))
    } else {
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
            let title = windows::core::w!("Clipboard Shelf");
            if let Ok(hwnd) = FindWindowW(None, title) {
                if !hwnd.0.is_null() {
                    APP_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
                    return Some(hwnd);
                }
            }
        }
        None
    }
}

fn release_mouse_and_notify_slint() {
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::Graphics::Gdi::ScreenToClient;
        use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW, WM_LBUTTONUP};
        use windows::Win32::Foundation::{POINT, WPARAM, LPARAM};

        let _ = ReleaseCapture();
        if let Some(hwnd) = get_app_hwnd() {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = ScreenToClient(hwnd, &mut pt);
            let lparam = ((pt.y as u32) << 16) | (pt.x as u32 & 0xffff);
            let _ = PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
            log_msg("--> Cleared mouse capture and posted synthetic WM_LBUTTONUP to Slint window");
        }
    }
}

pub fn log_msg(msg: impl AsRef<str>) {
    let line = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg.as_ref());
    println!("{}", line);
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("C:\\Users\\Admin\\clipboard-shelf\\app.log") {
        let _ = writeln!(file, "{}", line);
    }
}

pub fn check_has_more(text: &str) -> bool {
    let mut visual_lines = 0;
    for line in text.lines() {
        let chars = line.chars().count();
        let lines_for_this = if chars <= 38 {
            1
        } else {
            (chars + 37) / 38
        };
        visual_lines += lines_for_this;
        if visual_lines > 3 {
            return true;
        }
    }
    visual_lines > 3
}

fn refresh_ui_model(
    cards_model: &Rc<VecModel<ShelfCardData>>,
    items_store: &HashMap<String, ClipboardItem>,
    items_order: &[String],
    filter: &str,
    search: &str,
    ui_weak: &slint::Weak<MainWindow>,
) {
    let search_lower = search.trim().to_lowercase();
    let mut visible = Vec::new();

    for id in items_order {
        if let Some(item) = items_store.get(id) {
            // Category filter
            let matches_category = match filter {
                "text" => item.item_type == ItemType::Text,
                "url" => item.item_type == ItemType::Url,
                "files" => item.item_type == ItemType::Files,
                "image" => item.item_type == ItemType::Image,
                _ => true,
            };
            if !matches_category {
                continue;
            }

            // Search filter
            if !search_lower.is_empty() {
                let title_matches = item.title.to_lowercase().contains(&search_lower);
                let preview_matches = item.preview.to_lowercase().contains(&search_lower);
                let text_matches = item
                    .text_content
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&search_lower))
                    .unwrap_or(false);
                let file_matches = item
                    .file_paths
                    .iter()
                    .any(|f| f.to_lowercase().contains(&search_lower));

                if !title_matches && !preview_matches && !text_matches && !file_matches {
                    continue;
                }
            }

            let has_more = if item.item_type == ItemType::Image {
                true
            } else {
                check_has_more(&item.preview)
            };

            let thumbnail = if item.item_type == ItemType::Image && !item.file_paths.is_empty() {
                let p = std::path::Path::new(&item.file_paths[0]);
                if let Ok(img) = slint::Image::load_from_path(p) {
                    img
                } else if let Ok(dyn_img) = image::open(p) {
                    let rgba = dyn_img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let mut pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
                    pixel_buffer.make_mut_bytes().copy_from_slice(&rgba.into_raw());
                    slint::Image::from_rgba8(pixel_buffer)
                } else {
                    slint::Image::default()
                }
            } else {
                slint::Image::default()
            };

            let is_en = get_locale() == "en";
            let (loc_title, loc_preview) = localize_title_and_preview(&item.title, &item.preview, is_en);

            visible.push(ShelfCardData {
                id: item.id.clone().into(),
                title: loc_title.into(),
                preview: loc_preview.into(),
                icon: item.item_type.icon().into(),
                time: item.captured_at.format("%H:%M").to_string().into(),
                item_type: match item.item_type {
                    ItemType::Text => "text".into(),
                    ItemType::Url => "url".into(),
                    ItemType::Files => "files".into(),
                    ItemType::Image => "image".into(),
                    ItemType::Custom => "custom".into(),
                },
                pinned: item.pinned,
                has_more,
                thumbnail,
            });
        }
    }

    cards_model.set_vec(visible);
    if let Some(ui) = ui_weak.upgrade() {
        ui.set_item_count(cards_model.row_count() as i32);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    if let Ok(mut guard) = GLOBAL_UI_WEAK.lock() {
        guard.replace(ui.as_weak());
    }

    let initial_loc = std::fs::read_to_string("C:\\Users\\Admin\\clipboard-shelf\\locale.txt")
        .unwrap_or_else(|_| "vi".to_string())
        .trim()
        .to_string();
    let initial_loc = if initial_loc == "en" { "en" } else { "vi" };
    set_locale(initial_loc);
    ui.set_current_locale(initial_loc.into());

    ui.on_toggle_language(move || {
        toggle_locale();
    });

    ui.on_toggle_auto_hide(move || {
        toggle_auto_hide_mode();
    });

    // Shared thread-safe store for full clipboard items
    let items_store: Arc<Mutex<HashMap<String, ClipboardItem>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let items_order: Arc<Mutex<Vec<String>>> =
        Arc::new(Mutex::new(Vec::new()));
    let current_filter: Arc<Mutex<String>> =
        Arc::new(Mutex::new("all".to_string()));
    let current_search: Arc<Mutex<String>> =
        Arc::new(Mutex::new(String::new()));

    // Slint UI list model
    let cards_model: Rc<VecModel<ShelfCardData>> = Rc::new(VecModel::default());
    ui.set_items(ModelRc::from(cards_model.clone()));
    ui.set_item_count(0);

    // Setup callback to add item onto UI thread model
    let cards_model_for_add = cards_model.clone();
    let ui_weak_for_add = ui.as_weak();
    ui.on_add_shelf_item(move |card| {
        log_msg(format!("--> [UI Thread] Adding shelf card: id={}, title={}", card.id, card.title));
        cards_model_for_add.insert(0, card);
        if let Some(ui) = ui_weak_for_add.upgrade() {
            ui.set_item_count(cards_model_for_add.row_count() as i32);
            ui.set_selected_index(0);
        }
    });

    // Setup callback for live search and category filter changes
    let cards_model_for_filter = cards_model.clone();
    let items_store_for_filter = items_store.clone();
    let items_order_for_filter = items_order.clone();
    let current_filter_for_filter = current_filter.clone();
    let current_search_for_filter = current_search.clone();
    let ui_weak_for_filter = ui.as_weak();
    ui.on_filter_changed(move |filter, search| {
        log_msg(format!("Filter changed: filter={}, search={}", filter, search));
        *current_filter_for_filter.lock().unwrap() = filter.to_string();
        *current_search_for_filter.lock().unwrap() = search.to_string();
        let store = items_store_for_filter.lock().unwrap();
        let order = items_order_for_filter.lock().unwrap();
        refresh_ui_model(
            &cards_model_for_filter,
            &store,
            &order,
            filter.as_str(),
            search.as_str(),
            &ui_weak_for_filter,
        );
        if let Some(ui) = ui_weak_for_filter.upgrade() {
            ui.set_scroll_viewport_y(0.0);
        }
    });

    // Setup callback to refresh shelf model on UI thread
    let cards_model_for_refresh = cards_model.clone();
    let items_store_for_refresh = items_store.clone();
    let items_order_for_refresh = items_order.clone();
    let current_filter_for_refresh = current_filter.clone();
    let current_search_for_refresh = current_search.clone();
    let ui_weak_for_refresh = ui.as_weak();
    ui.on_refresh_shelf(move || {
        let store = items_store_for_refresh.lock().unwrap();
        let order = items_order_for_refresh.lock().unwrap();
        let filter = current_filter_for_refresh.lock().unwrap();
        let search = current_search_for_refresh.lock().unwrap();
        refresh_ui_model(
            &cards_model_for_refresh,
            &store,
            &order,
            &filter,
            &search,
            &ui_weak_for_refresh,
        );
    });

    // Start Win32 background clipboard listener
    let (tx, rx) = unbounded::<ClipboardItem>();
    let _listener = ClipboardListener::start(tx);

    // Receive captured items and forward to Slint event loop
    let ui_weak = ui.as_weak();
    let items_store_for_bg = items_store.clone();
    let items_order_for_bg = items_order.clone();
    std::thread::spawn(move || {
        println!("--> [Worker Thread] Waiting for clipboard items...");
        while let Ok(item) = rx.recv() {
            let store = items_store_for_bg.lock().unwrap();
            let mut order = items_order_for_bg.lock().unwrap();
            // Insert after all currently pinned items so pinned items always remain on top
            let insert_pos = order
                .iter()
                .position(|id| store.get(id).map(|i| !i.pinned).unwrap_or(true))
                .unwrap_or(order.len());
            drop(store);

            items_store_for_bg.lock().unwrap().insert(item.id.clone(), item.clone());
            order.insert(insert_pos, item.id.clone());
            drop(order);

            let ui_weak = ui_weak.clone();
            let res = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_refresh_shelf();
                    ui.set_selected_index(insert_pos as i32);
                    ui.set_scroll_viewport_y(0.0);
                }
            });
            if let Err(e) = res {
                eprintln!("--> [Worker Thread] invoke_from_event_loop error: {:?}", e);
            }

            if AUTO_HIDE_ENABLED.load(Ordering::SeqCst) {
                TARGET_WINDOW_X.store(EXPANDED_WINDOW_X.load(Ordering::SeqCst), Ordering::SeqCst);
                IS_COLLAPSED.store(false, Ordering::SeqCst);
                LAST_INSIDE_MS.store(chrono::Local::now().timestamp_millis() + 2000, Ordering::SeqCst);
            }
        }
    });

    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
    }

    // 1. Native OLE Drag & Drop callback
    let items_store_for_drag = items_store.clone();
    let cards_model_for_drag = cards_model.clone();
    let ui_weak_for_drag = ui.as_weak();
    static DRAGGING: AtomicBool = AtomicBool::new(false);
    static LAST_DRAG_MS: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

    ui.on_start_drag_item(move |id, idx| {
        let now_ms = chrono::Local::now().timestamp_millis();
        let last_ms = LAST_DRAG_MS.load(Ordering::SeqCst);
        if (now_ms - last_ms).abs() < 300 {
            log_msg(format!("Ignored rapid re-drag within cooldown ({}ms < 300ms)", now_ms - last_ms));
            return;
        }

        // Verify left mouse button is physically held down
        let lbutton_down = unsafe {
            (windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x01) as i16) < 0
        };
        if !lbutton_down {
            log_msg("Ignored drag trigger: left mouse button is not physically pressed");
            return;
        }

        if DRAGGING
            .compare_exchange(
                false,
                true,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return;
        }

        // Live ground truth: obtain row from model by index!
        let target_id = cards_model_for_drag
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        let store = items_store_for_drag.lock().unwrap();
        let item_opt = store.get(&target_id).cloned().or_else(|| store.get(id.as_str()).cloned());
        drop(store);

        if let Some(item) = item_opt {
            log_msg(format!("Starting OLE Drag for [row {}]: target_id={}, title={}", idx, target_id, item.title));
            if let Some(ui) = ui_weak_for_drag.upgrade() {
                ui.set_is_ole_dragging(true);
                ui.set_ole_dragging_index(idx);
            }

            let ui_weak_timer = ui_weak_for_drag.clone();
            // Single-shot timer (30ms) allows Slint to render the active dragging overlay banner before blocking in DoDragDrop!
            slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
                let effect = transfer::initiate_drag_drop(item.text_content.clone(), item.file_paths.clone());
                log_msg(format!("Drag completed for [row {}] with effect: {}", idx, effect));

                LAST_DRAG_MS.store(chrono::Local::now().timestamp_millis(), Ordering::SeqCst);

                if let Some(ui) = ui_weak_timer.upgrade() {
                    ui.set_is_ole_dragging(false);
                    ui.set_ole_dragging_index(-1);
                }

                release_mouse_and_notify_slint();
                DRAGGING.store(false, Ordering::SeqCst);
            });
        } else {
            LAST_DRAG_MS.store(chrono::Local::now().timestamp_millis(), Ordering::SeqCst);
            if let Some(ui) = ui_weak_for_drag.upgrade() {
                ui.set_is_ole_dragging(false);
                ui.set_ole_dragging_index(-1);
            }
            DRAGGING.store(false, Ordering::SeqCst);
        }
    });

    // 2. Pin item callback
    let cards_model_for_pin = cards_model.clone();
    let items_store_for_pin = items_store.clone();
    let items_order_for_pin = items_order.clone();
    let current_filter_for_pin = current_filter.clone();
    let current_search_for_pin = current_search.clone();
    let ui_weak_for_pin = ui.as_weak();
    ui.on_pin_item(move |id, idx| {
        let target_id = cards_model_for_pin
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        let mut store = items_store_for_pin.lock().unwrap();
        let mut order = items_order_for_pin.lock().unwrap();
        if let Some(item) = store.get_mut(&target_id) {
            item.pinned = !item.pinned;
            let is_pinned = item.pinned;

            if is_pinned {
                // When pinned, jump immediately to the very top (index 0) of the shelf!
                if let Some(pos) = order.iter().position(|x| x == &target_id) {
                    let val = order.remove(pos);
                    order.insert(0, val);
                }
            }

            let filter = current_filter_for_pin.lock().unwrap();
            let search = current_search_for_pin.lock().unwrap();
            refresh_ui_model(
                &cards_model_for_pin,
                &store,
                &order,
                &filter,
                &search,
                &ui_weak_for_pin,
            );

            if is_pinned {
                if let Some(ui) = ui_weak_for_pin.upgrade() {
                    ui.set_selected_index(0);
                    ui.set_scroll_viewport_y(0.0);
                }
            }
        }
    });

    // 3. Delete item callback
    let cards_model_for_del = cards_model.clone();
    let items_store_for_del = items_store.clone();
    let items_order_for_del = items_order.clone();
    let current_filter_for_del = current_filter.clone();
    let current_search_for_del = current_search.clone();
    let ui_weak_for_del = ui.as_weak();
    ui.on_delete_item(move |id, idx| {
        let target_id = cards_model_for_del
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        items_store_for_del.lock().unwrap().remove(&target_id);
        items_order_for_del.lock().unwrap().retain(|x| x != &target_id);

        let store = items_store_for_del.lock().unwrap();
        let order = items_order_for_del.lock().unwrap();
        let filter = current_filter_for_del.lock().unwrap();
        let search = current_search_for_del.lock().unwrap();
        refresh_ui_model(
            &cards_model_for_del,
            &store,
            &order,
            &filter,
            &search,
            &ui_weak_for_del,
        );
    });

    // 4. Clear all unpinned items
    let cards_model_for_clear = cards_model.clone();
    let items_store_for_clear = items_store.clone();
    let items_order_for_clear = items_order.clone();
    let current_filter_for_clear = current_filter.clone();
    let current_search_for_clear = current_search.clone();
    let ui_weak_for_clear = ui.as_weak();
    ui.on_clear_all(move || {
        let mut store = items_store_for_clear.lock().unwrap();
        let mut order = items_order_for_clear.lock().unwrap();

        store.retain(|_, item| item.pinned);
        order.retain(|id| store.contains_key(id));

        let filter = current_filter_for_clear.lock().unwrap();
        let search = current_search_for_clear.lock().unwrap();
        refresh_ui_model(
            &cards_model_for_clear,
            &store,
            &order,
            &filter,
            &search,
            &ui_weak_for_clear,
        );
    });

    // 5. Move item callback (reorder items)
    let cards_model_for_move = cards_model.clone();
    let items_store_for_move = items_store.clone();
    let items_order_for_move = items_order.clone();
    let current_filter_for_move = current_filter.clone();
    let current_search_for_move = current_search.clone();
    let ui_weak_for_move = ui.as_weak();
    ui.on_move_item(move |from_idx, to_idx| {
        let count = cards_model_for_move.row_count();
        if from_idx < 0
            || to_idx < 0
            || from_idx as usize >= count
            || to_idx as usize >= count
            || from_idx == to_idx
        {
            return;
        }

        let from_id = cards_model_for_move.row_data(from_idx as usize).map(|c| c.id.to_string());
        let to_id = cards_model_for_move.row_data(to_idx as usize).map(|c| c.id.to_string());

        if let (Some(f_id), Some(t_id)) = (from_id, to_id) {
            let mut order = items_order_for_move.lock().unwrap();
            let f_pos = order.iter().position(|x| x == &f_id);
            let t_pos = order.iter().position(|x| x == &t_id);
            if let (Some(fp), Some(tp)) = (f_pos, t_pos) {
                let val = order.remove(fp);
                order.insert(tp, val);
            }
            drop(order);

            let store = items_store_for_move.lock().unwrap();
            let order = items_order_for_move.lock().unwrap();
            let filter = current_filter_for_move.lock().unwrap();
            let search = current_search_for_move.lock().unwrap();
            refresh_ui_model(
                &cards_model_for_move,
                &store,
                &order,
                &filter,
                &search,
                &ui_weak_for_move,
            );
            if let Some(ui) = ui_weak_for_move.upgrade() {
                ui.set_selected_index(to_idx);
            }
            log_msg(format!("Reordered item {} to position {}", f_id, to_idx));
        }
    });

    // 6. Smooth drag-to-reorder auto-scroll timer
    let ui_weak_scroll = ui.as_weak();
    let scroll_timer = Box::leak(Box::new(slint::Timer::default()));
    scroll_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(30),
        move || {
            if let Some(ui) = ui_weak_scroll.upgrade() {
                if ui.get_is_reordering() {
                    let vy = ui.get_drag_mouse_viewport_y();
                    if vy >= 0.0 {
                        let cur_scroll = ui.get_scroll_viewport_y();
                        let item_count = ui.get_item_count();
                        let max_scroll = ((item_count as f32 * 132.0) - 380.0).max(0.0);

                        // Top auto-scroll zone: when cursor is near or above top edge
                        if vy < 70.0 {
                            let speed = if vy < 20.0 { 22.0 } else { 12.0 };
                            let new_scroll = (cur_scroll + speed).min(0.0);
                            if new_scroll != cur_scroll {
                                ui.set_scroll_viewport_y(new_scroll);
                                let delta_from_top = -new_scroll;
                                let target_idx = ((delta_from_top + vy) / 132.0).round() as i32;
                                let clamped = target_idx.clamp(0, item_count - 1);
                                ui.set_reorder_to(clamped);
                            }
                        } else if vy > 380.0 {
                            // Bottom auto-scroll zone: when cursor is near or below bottom edge
                            let speed = if vy > 430.0 { 22.0 } else { 12.0 };
                            let new_scroll = (cur_scroll - speed).max(-max_scroll);
                            if new_scroll != cur_scroll {
                                ui.set_scroll_viewport_y(new_scroll);
                                let delta_from_top = -new_scroll;
                                let target_idx = ((delta_from_top + vy) / 132.0).round() as i32;
                                let clamped = target_idx.clamp(0, item_count - 1);
                                ui.set_reorder_to(clamped);
                            }
                        }
                    }
                }
            }
        },
    );

    // Copy item back to clipboard callback when clicked
    let items_store_for_copy = items_store.clone();
    let cards_model_for_copy = cards_model.clone();
    let ui_weak_for_copy = ui.as_weak();
    ui.on_copy_item(move |id, idx| {
        let target_id = cards_model_for_copy
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        if let Some(ui) = ui_weak_for_copy.upgrade() {
            ui.set_copied_flash_index(idx);
            let ui_weak_timer = ui_weak_for_copy.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(1500), move || {
                if let Some(ui) = ui_weak_timer.upgrade() {
                    if ui.get_copied_flash_index() == idx {
                        ui.set_copied_flash_index(-1);
                    }
                }
            });
        }

        if let Some(item) = items_store_for_copy.lock().unwrap().get(&target_id) {
            log_msg(format!("Card clicked! Re-copying to clipboard: [row {}] target_id={}, title={}", idx, target_id, item.title));
            if let Some(text) = &item.text_content {
                crate::clipboard::set_internal_copy();
                unsafe {
                    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
                    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
                    use windows::Win32::Foundation::{HANDLE, HWND};

                    if OpenClipboard(HWND::default()).is_ok() {
                        let _ = EmptyClipboard();
                        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
                            let ptr = GlobalLock(hmem);
                            if !ptr.is_null() {
                                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
                                let _ = GlobalUnlock(hmem);
                                let _ = SetClipboardData(13, HANDLE(hmem.0));
                            }
                        }
                        let _ = CloseClipboard();
                    }
                }
            } else if !item.file_paths.is_empty() {
                crate::clipboard::set_internal_copy();
                if let Some(hdrop) = crate::transfer::create_hdrop_from_paths(&item.file_paths) {
                    unsafe {
                        use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
                        use windows::Win32::Foundation::{HANDLE, HWND};

                        if OpenClipboard(HWND::default()).is_ok() {
                            let _ = EmptyClipboard();
                            let _ = SetClipboardData(15, HANDLE(hdrop.0));
                            let _ = CloseClipboard();
                        }
                    }
                }
            }
        }
    });

    // 7. Auto-Hide Edge Dock and Tray Event Timer
    let ui_weak_edge = ui.as_weak();
    let edge_timer = Box::leak(Box::new(slint::Timer::default()));
    edge_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(25),
        move || {
            // Process tray clear all signal if triggered
            if crate::tray::TRAY_CLEAR_TRIGGERED.swap(false, Ordering::SeqCst) {
                if let Some(ui) = ui_weak_edge.upgrade() {
                    ui.invoke_clear_all();
                }
            }

            // If window is hidden from tray, do not process auto-hide / slide
            if !IS_WINDOW_VISIBLE.load(Ordering::SeqCst) {
                return;
            }

            let auto_hide = AUTO_HIDE_ENABLED.load(Ordering::SeqCst);
            let cur_x = CURRENT_WINDOW_X.load(Ordering::SeqCst);
            let exp_x = EXPANDED_WINDOW_X.load(Ordering::SeqCst);
            let col_x = COLLAPSED_WINDOW_X.load(Ordering::SeqCst);
            let win_y = WINDOW_Y.load(Ordering::SeqCst);
            let win_w = WINDOW_W.load(Ordering::SeqCst);
            let win_h = WINDOW_H.load(Ordering::SeqCst);
            let wa_right = WORK_AREA_RIGHT.load(Ordering::SeqCst);
            let now_ms = chrono::Local::now().timestamp_millis();

            if auto_hide {
                let mut cursor_pt = windows::Win32::Foundation::POINT::default();
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor_pt);
                }

                let is_dragging = if let Some(ui) = ui_weak_edge.upgrade() {
                    ui.get_is_reordering() || ui.get_is_ole_dragging()
                } else {
                    false
                };

                let inside_win = cursor_pt.x >= cur_x
                    && cursor_pt.x <= cur_x + win_w
                    && cursor_pt.y >= win_y
                    && cursor_pt.y <= win_y + win_h;

                let hovering_tab = cursor_pt.x >= (wa_right - 28)
                    && cursor_pt.y >= win_y
                    && cursor_pt.y <= win_y + win_h;

                if inside_win || hovering_tab || is_dragging {
                    LAST_INSIDE_MS.store(now_ms, Ordering::SeqCst);
                    TARGET_WINDOW_X.store(exp_x, Ordering::SeqCst);
                    IS_COLLAPSED.store(false, Ordering::SeqCst);
                } else {
                    let last_ms = LAST_INSIDE_MS.load(Ordering::SeqCst);
                    if now_ms - last_ms > 350 {
                        TARGET_WINDOW_X.store(col_x, Ordering::SeqCst);
                        IS_COLLAPSED.store(true, Ordering::SeqCst);
                    }
                }
            } else {
                TARGET_WINDOW_X.store(exp_x, Ordering::SeqCst);
                IS_COLLAPSED.store(false, Ordering::SeqCst);
            }

            // Smooth slide animation (exponential ease-out step)
            let tgt_x = TARGET_WINDOW_X.load(Ordering::SeqCst);
            if cur_x != tgt_x {
                let diff = tgt_x - cur_x;
                let step = if diff.abs() <= 5 {
                    diff
                } else {
                    ((diff as f32) * 0.40).round() as i32
                };
                let new_x = cur_x + step;
                CURRENT_WINDOW_X.store(new_x, Ordering::SeqCst);

                if let Some(hwnd) = get_app_hwnd() {
                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::*;
                        let _ = SetWindowPos(
                            hwnd,
                            windows::Win32::Foundation::HWND(-1 as _),
                            new_x,
                            win_y,
                            win_w,
                            win_h,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            }
        },
    );

    // Enforce right edge-docked placement and topmost on Windows desktop
    std::thread::spawn(move || {
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::*;
                use windows::Win32::Foundation::{HWND, RECT};

                let title = windows::core::w!("Clipboard Shelf");
                if let Ok(hwnd) = FindWindowW(None, title) {
                    if !hwnd.0.is_null() {
                        APP_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

                        let mut wa = RECT::default();
                        let ok = SystemParametersInfoW(
                            SPI_GETWORKAREA,
                            0,
                            Some(&mut wa as *mut _ as *mut core::ffi::c_void),
                            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                        );
                        let (_screen_w, screen_h) = if ok.is_ok() && wa.right > wa.left {
                            (wa.right - wa.left, wa.bottom - wa.top)
                        } else {
                            let w = GetSystemMetrics(SM_CXSCREEN);
                            let h = GetSystemMetrics(SM_CYSCREEN);
                            wa.left = 0;
                            wa.top = 0;
                            wa.right = w;
                            wa.bottom = h;
                            (w, h)
                        };

                        let win_w = 428;
                        let win_h = 640.min(screen_h - 40);
                        let win_y = wa.top + ((screen_h - win_h) / 2).max(10);
                        let exp_x = wa.right - win_w;
                        let col_x = wa.right - 14;

                        EXPANDED_WINDOW_X.store(exp_x, Ordering::SeqCst);
                        COLLAPSED_WINDOW_X.store(col_x, Ordering::SeqCst);
                        CURRENT_WINDOW_X.store(exp_x, Ordering::SeqCst);
                        TARGET_WINDOW_X.store(exp_x, Ordering::SeqCst);
                        WINDOW_Y.store(win_y, Ordering::SeqCst);
                        WINDOW_W.store(win_w, Ordering::SeqCst);
                        WINDOW_H.store(win_h, Ordering::SeqCst);
                        WORK_AREA_RIGHT.store(wa.right, Ordering::SeqCst);
                        WORK_AREA_TOP.store(wa.top, Ordering::SeqCst);
                        WORK_AREA_BOTTOM.store(wa.bottom, Ordering::SeqCst);
                        LAST_INSIDE_MS.store(chrono::Local::now().timestamp_millis(), Ordering::SeqCst);

                        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                        let _ = SetWindowPos(
                            hwnd,
                            HWND(-1 as _), // HWND_TOPMOST
                            exp_x,
                            win_y,
                            win_w,
                            win_h,
                            SWP_SHOWWINDOW,
                        );
                        let _ = SetForegroundWindow(hwnd);
                        println!(
                            "--> Docked window to right screen edge: ({}, {}) size: {}x{}, workarea right: {}",
                            exp_x, win_y, win_w, win_h, wa.right
                        );
                        break;
                    }
                }
            }
        }
    });

    println!("Clipboard Shelf UI initialized successfully!");
    ui.show()?;
    ui.run()?;

    Ok(())
}
