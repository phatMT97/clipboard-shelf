mod domain;
mod clipboard;
mod transfer;
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
use std::sync::{Arc, Mutex};

slint::include_modules!();

pub fn log_msg(msg: impl AsRef<str>) {
    let line = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S%.3f"), msg.as_ref());
    println!("{}", line);
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("C:\\Users\\Admin\\clipboard-shelf\\app.log") {
        let _ = writeln!(file, "{}", line);
    }
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

            visible.push(ShelfCardData {
                id: item.id.clone().into(),
                title: item.title.clone().into(),
                preview: item.preview.clone().into(),
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
            println!("--> [Worker Thread] rx.recv() got: {}", item.title);
            items_store_for_bg.lock().unwrap().insert(item.id.clone(), item.clone());
            items_order_for_bg.lock().unwrap().insert(0, item.id.clone());

            let ui_weak = ui_weak.clone();
            let res = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_refresh_shelf();
                    ui.set_selected_index(0);
                }
            });
            if let Err(e) = res {
                eprintln!("--> [Worker Thread] invoke_from_event_loop error: {:?}", e);
            }
        }
    });

    unsafe {
        let _ = windows::Win32::System::Ole::OleInitialize(None);
    }

    // 1. Native OLE Drag & Drop callback
    let items_store_for_drag = items_store.clone();
    let cards_model_for_drag = cards_model.clone();
    static DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    ui.on_start_drag_item(move |id, idx| {
        if DRAGGING
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
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
        if let Some(item) = store.get(&target_id) {
            log_msg(format!("Initiating native Windows OLE Drag for [row {}]: target_id={}, title={}", idx, target_id, item.title));
            let effect = transfer::initiate_drag_drop(item.text_content.clone(), item.file_paths.clone());
            log_msg(format!("Drag completed for [row {}] with effect: {}", idx, effect));
        } else if let Some(item) = store.get(id.as_str()) {
            log_msg(format!("Fallback Drag for [id {}]: title={}", id, item.title));
            let effect = transfer::initiate_drag_drop(item.text_content.clone(), item.file_paths.clone());
            log_msg(format!("Drag completed for [id {}] with effect: {}", id, effect));
        }

        DRAGGING.store(false, std::sync::atomic::Ordering::SeqCst);
    });

    // 2. Pin item callback
    let cards_model_for_pin = cards_model.clone();
    let items_store_for_pin = items_store.clone();
    ui.on_pin_item(move |id, idx| {
        let target_id = cards_model_for_pin
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        let mut store = items_store_for_pin.lock().unwrap();
        if let Some(item) = store.get_mut(&target_id) {
            item.pinned = !item.pinned;
            let pinned_state = item.pinned;

            if let Some(mut card) = cards_model_for_pin.row_data(idx as usize) {
                card.pinned = pinned_state;
                cards_model_for_pin.set_row_data(idx as usize, card);
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

    // Copy item back to clipboard callback when clicked
    let items_store_for_copy = items_store.clone();
    let cards_model_for_copy = cards_model.clone();
    ui.on_copy_item(move |id, idx| {
        let target_id = cards_model_for_copy
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

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
            }
        }
    });

    // Enforce window visibility and topmost placement on Windows desktop
    std::thread::spawn(move || {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::*;
                use windows::Win32::Foundation::HWND;

                let title = windows::core::w!("Clipboard Shelf");
                if let Ok(hwnd) = FindWindowW(None, title) {
                    if !hwnd.0.is_null() {
                        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                        let _ = SetWindowPos(
                            hwnd,
                            HWND(-1 as _), // HWND_TOPMOST
                            100,
                            100,
                            360,
                            580,
                            SWP_SHOWWINDOW,
                        );
                        let _ = SetForegroundWindow(hwnd);
                        println!("--> Found window! Enforced HWND_TOPMOST, position (100, 100), and foreground!");
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
