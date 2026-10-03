#[cfg(test)]
mod tests {
    use crate::transfer::ShelfDataObject;
    use windows::Win32::Foundation::{S_FALSE, S_OK};
    use windows::Win32::System::Com::{FORMATETC, IDataObject};

    #[test]
    fn test_data_object_formats() {
        let text = Some("Hello Drag & Drop".to_string());
        let files = vec!["C:\\test.txt".to_string()];
        let obj = ShelfDataObject::new(text, files);

        // Test EnumFormatEtc
        let data_obj: IDataObject = obj.into();
        let enum_fmt = unsafe { data_obj.EnumFormatEtc(1).expect("EnumFormatEtc must succeed") };

        let mut rgelt = [FORMATETC::default(); 5];
        let mut fetched = 0;
        let hr = unsafe { enum_fmt.Next(&mut rgelt, Some(&mut fetched)) };
        assert!(hr == S_OK || hr == S_FALSE);
        assert_eq!(fetched, 3, "Must enumerate 3 formats: CF_HDROP, CF_UNICODETEXT, CF_TEXT");

        println!("Successfully enumerated {} formats!", fetched);
    }

    #[test]
    fn test_drop_source_flags() {
        use windows::Win32::Foundation::{BOOL, DRAGDROP_S_DROP, S_OK};
        use windows::Win32::System::Ole::IDropSource;
        use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
        use crate::transfer::ShelfDropSource;

        let source: IDropSource = ShelfDropSource.into();
        unsafe {
            // When LBUTTON is down (0x0001)
            let res = source.QueryContinueDrag(BOOL(0), MODIFIERKEYS_FLAGS(1));
            assert_eq!(res, S_OK);

            // When LBUTTON is released (0)
            let res = source.QueryContinueDrag(BOOL(0), MODIFIERKEYS_FLAGS(0));
            assert_eq!(res, DRAGDROP_S_DROP);
        }
    }
}
