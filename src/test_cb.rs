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
        });

        cards_model.push(ShelfCardData {
            id: "ID_SECOND".into(),
            title: "Second".into(),
            preview: "Second".into(),
            icon: "📝".into(),
            time: "10:01".into(),
            item_type: "text".into(),
            pinned: false,
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
    }
}
