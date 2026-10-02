#[cfg(test)]
mod clipboard_test {
    use crate::clipboard::ClipboardListener;
    use crossbeam_channel::unbounded;
    use std::time::Duration;

    #[test]
    fn test_clipboard_capture() {
        let (tx, rx) = unbounded();
        let _listener = ClipboardListener::start(tx);
        std::thread::sleep(Duration::from_millis(200));

        // Copy test string to clipboard
        let test_text = format!("Unit test clipboard string {}", uuid::Uuid::new_v4());
        println!("Setting clipboard to: {}", test_text);
        
        let mut success = false;
        unsafe {
            use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
            use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
            use windows::Win32::Foundation::HWND;

            if OpenClipboard(HWND::default()).is_ok() {
                let _ = EmptyClipboard();
                let wide: Vec<u16> = test_text.encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
                    let ptr = GlobalLock(hmem);
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
                        let _ = GlobalUnlock(hmem);
                        let _ = SetClipboardData(13, windows::Win32::Foundation::HANDLE(hmem.0));
                        success = true;
                    }
                }
                let _ = CloseClipboard();
            }
        }

        assert!(success, "Must set clipboard successfully");

        // Now see if listener received it within 3 seconds
        let start = std::time::Instant::now();
        let mut found = false;
        while start.elapsed() < Duration::from_secs(3) {
            if let Ok(item) = rx.recv_timeout(Duration::from_millis(500)) {
                println!("Received item: {:?}", item.title);
                if item.text_content == Some(test_text.clone()) {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Must receive the generated test string from clipboard");
    }
}
