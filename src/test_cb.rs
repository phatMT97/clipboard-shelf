#[cfg(test)]
mod tests {
    use crate::{MainWindow, ShelfCardData};
    use slint::{ComponentHandle, Model, ModelRc, VecModel};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_slint_callbacks_and_reorder() {
        let ui = MainWindow::new().unwrap();
        let cards_model: Rc<VecModel<ShelfCardData>> = Rc::new(VecModel::default());
        ui.set_items(ModelRc::from(cards_model.clone()));

        cards_model.push(ShelfCardData {
            id: "ID_FIRST".into(),
            title: "First".into(),
            preview: "First".into(),
            icon: "📝".into(),
            time: "10:00".into(),
            item_type: "text".into(),
            pinned: false,
            has_more: false,
            thumbnail: slint::Image::default(),
        });

        cards_model.push(ShelfCardData {
            id: "ID_SECOND".into(),
            title: "Second".into(),
            preview: "Second".into(),
            icon: "📝".into(),
            time: "10:01".into(),
            item_type: "text".into(),
            pinned: false,
            has_more: false,
            thumbnail: slint::Image::default(),
        });



        let received_ids = Rc::new(RefCell::new(Vec::new()));
        let received_ids_clone = received_ids.clone();

        ui.on_start_drag_item(move |id, idx| {
            received_ids_clone.borrow_mut().push((id.to_string(), idx));
        });

        // Test start_drag_item directly with ID_SECOND and index 1
        ui.invoke_start_drag_item("ID_SECOND".into(), 1);
        assert_eq!(received_ids.borrow()[0], ("ID_SECOND".to_string(), 1));

        // Test reorder: move row 1 to row 0
        let cards_model_for_move = cards_model.clone();
        let ui_weak = ui.as_weak();
        ui.on_move_item(move |from_idx, to_idx| {
            let from = from_idx as usize;
            let to = to_idx as usize;
            if let Some(card) = cards_model_for_move.row_data(from) {
                cards_model_for_move.remove(from);
                cards_model_for_move.insert(to, card);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_selected_index(to_idx);
                }
            }
        });

        ui.invoke_move_item(1, 0);

        assert_eq!(cards_model.row_data(0).unwrap().id, "ID_SECOND");
        assert_eq!(cards_model.row_data(1).unwrap().id, "ID_FIRST");
        assert_eq!(ui.get_selected_index(), 0);

        // Test filter_changed callback
        let filter_received = Rc::new(RefCell::new(Vec::new()));
        let filter_received_clone = filter_received.clone();
        ui.on_filter_changed(move |filter, search| {
            filter_received_clone.borrow_mut().push((filter.to_string(), search.to_string()));
        });
        ui.invoke_filter_changed("text".into(), "hello".into());
        assert_eq!(filter_received.borrow()[0], ("text".to_string(), "hello".to_string()));

        // Test new UI properties: copied flash, expand toggles, OLE drag state
        assert_eq!(ui.get_copied_flash_index(), -1);
        ui.set_copied_flash_index(0);
        assert_eq!(ui.get_copied_flash_index(), 0);

        assert_eq!(ui.get_show_all_expanded(), false);
        ui.set_show_all_expanded(true);
        assert_eq!(ui.get_show_all_expanded(), true);

        assert_eq!(ui.get_is_ole_dragging(), false);
        ui.set_is_ole_dragging(true);
        ui.set_ole_dragging_index(1);
        assert_eq!(ui.get_is_ole_dragging(), true);
        assert_eq!(ui.get_ole_dragging_index(), 1);
        ui.set_is_ole_dragging(false);
        ui.set_ole_dragging_index(-1);
        assert_eq!(ui.get_is_ole_dragging(), false);
        assert_eq!(ui.get_ole_dragging_index(), -1);

        assert_eq!(ui.get_drag_mouse_viewport_y(), -1.0);
        ui.set_drag_mouse_viewport_y(45.0);
        assert_eq!(ui.get_drag_mouse_viewport_y(), 45.0);
    }

    #[test]
    fn test_pin_item_jump_to_top() {
        let mut order = vec!["id_1".to_string(), "id_2".to_string(), "id_3".to_string()];
        // Pin id_3 -> should jump to index 0
        let target_id = "id_3".to_string();
        if let Some(pos) = order.iter().position(|x| x == &target_id) {
            let val = order.remove(pos);
            order.insert(0, val);
        }
        assert_eq!(order, vec!["id_3", "id_1", "id_2"]);
    }

    #[test]
    fn test_check_has_more() {
        use crate::check_has_more;
        // 1-2 short lines should fit without expand button
        assert_eq!(check_has_more("Hello world"), false);
        assert_eq!(check_has_more("Unit test clipboard string\n4339a39a-7dba-480b-98c5-88cfdb8c1ecd"), false);
        assert_eq!(check_has_more("Line 1\nLine 2\nLine 3"), false);

        // 4 lines or very long paragraph should require expand button
        assert_eq!(check_has_more("Line 1\nLine 2\nLine 3\nLine 4"), true);
        let long_line = "This is a very long string that will definitely wrap across multiple lines because it exceeds the standard card content line width by a significant margin.";
        assert_eq!(check_has_more(long_line), true);
    }

    #[test]
    fn test_pixel_buffer_fallback() {
        let temp_dir = std::env::temp_dir().join("clipboard-shelf");
        let sample_bmp = temp_dir.join("image_20261003_123925.bmp");
        if sample_bmp.exists() {
            let dyn_img = image::open(&sample_bmp).expect("image::open failed");
            let rgba = dyn_img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
            pixel_buffer.make_mut_bytes().copy_from_slice(&rgba.into_raw());
            let img = slint::Image::from_rgba8(pixel_buffer);
            assert_eq!(img.size().width, w);
            assert_eq!(img.size().height, h);
            println!("Fallback SharedPixelBuffer success! Size: {}x{}", w, h);
        }
    }

    #[test]
    fn test_tray_types() {
        use windows::Win32::UI::Shell::NOTIFYICONDATAW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreatePopupMenu, AppendMenuW, DestroyMenu,
            MF_STRING, MF_SEPARATOR,
        };
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            ..Default::default()
        };
        assert_eq!(nid.cbSize as usize, std::mem::size_of::<NOTIFYICONDATAW>());

        unsafe {
            if let Ok(hmenu) = CreatePopupMenu() {
                let _ = AppendMenuW(hmenu, MF_STRING, 101, windows::core::w!("Open"));
                let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(hmenu, MF_STRING, 102, windows::core::w!("Exit"));
                let _ = DestroyMenu(hmenu);
            }
        }
    }
}

