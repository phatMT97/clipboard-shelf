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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;

    // Shared thread-safe store for full clipboard items
    let items_store: Arc<Mutex<HashMap<String, ClipboardItem>>> =
        Arc::new(Mutex::new(HashMap::new()));

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

    // Start Win32 background clipboard listener
    let (tx, rx) = unbounded::<ClipboardItem>();
    let _listener = ClipboardListener::start(tx);

    // Receive captured items and forward to Slint event loop
    let ui_weak = ui.as_weak();
    let items_store_for_bg = items_store.clone();
    std::thread::spawn(move || {
        println!("--> [Worker Thread] Waiting for clipboard items...");
        while let Ok(item) = rx.recv() {
            println!("--> [Worker Thread] rx.recv() got: {}", item.title);
            let card = ShelfCardData {
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
            };

            items_store_for_bg.lock().unwrap().insert(item.id.clone(), item);

            let ui_weak = ui_weak.clone();
            let res = slint::invoke_from_event_loop(move || {
                println!("--> [Event Loop] invoke_from_event_loop running for: {}", card.title);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_add_shelf_item(card);
                } else {
                    eprintln!("--> [Event Loop] Failed to upgrade ui_weak!");
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
    let ui_weak_for_del = ui.as_weak();
    ui.on_delete_item(move |id, idx| {
        let target_id = cards_model_for_del
            .row_data(idx as usize)
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| id.to_string());

        items_store_for_del.lock().unwrap().remove(&target_id);
        cards_model_for_del.remove(idx as usize);

        if let Some(ui) = ui_weak_for_del.upgrade() {
            ui.set_item_count(cards_model_for_del.row_count() as i32);
        }
    });

    // 4. Clear all unpinned items
    let cards_model_for_clear = cards_model.clone();
    let items_store_for_clear = items_store.clone();
    let ui_weak_for_clear = ui.as_weak();
    ui.on_clear_all(move || {
        let mut store = items_store_for_clear.lock().unwrap();
        let mut pinned_ids = Vec::new();
        for (id, item) in store.iter() {
            if item.pinned {
                pinned_ids.push(id.clone());
            }
        }

        store.retain(|id, _| pinned_ids.contains(id));

        let mut idx = 0;
        while idx < cards_model_for_clear.row_count() {
            if let Some(card) = cards_model_for_clear.row_data(idx) {
                if !card.pinned {
                    cards_model_for_clear.remove(idx);
                    continue;
                }
            }
            idx += 1;
        }

        if let Some(ui) = ui_weak_for_clear.upgrade() {
            ui.set_item_count(cards_model_for_clear.row_count() as i32);
        }
    });

    // 5. Move item callback (reorder items)
    let cards_model_for_move = cards_model.clone();
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
        let from = from_idx as usize;
        let to = to_idx as usize;
        if let Some(card) = cards_model_for_move.row_data(from) {
            cards_model_for_move.remove(from);
            cards_model_for_move.insert(to, card);
            if let Some(ui) = ui_weak_for_move.upgrade() {
                ui.set_selected_index(to_idx);
            }
            log_msg(format!("Reordered item from position {} to {}", from_idx, to_idx));
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
                            340,
                            560,
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
