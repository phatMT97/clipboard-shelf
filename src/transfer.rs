use std::mem::{size_of, ManuallyDrop};
use std::ptr::copy_nonoverlapping;
use windows::core::{implement, HRESULT};
use windows::Win32::Foundation::{
    BOOL, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC,
    DV_E_TYMED, E_INVALIDARG, E_NOTIMPL, HGLOBAL, POINT, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumFORMATETC_Impl, IEnumSTATDATA,
    DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::DROPFILES;

const MK_LBUTTON: u32 = 0x0001;
const CF_TEXT: u16 = 1;
const CF_UNICODETEXT: u16 = 13;
const CF_HDROP: u16 = 15;

#[allow(non_snake_case)]
#[implement(IDropSource)]
pub struct ShelfDropSource;

impl IDropSource_Impl for ShelfDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        fescapepressed: BOOL,
        grfkeystate: MODIFIERKEYS_FLAGS,
    ) -> HRESULT {
        if fescapepressed.as_bool() {
            crate::log_msg("QueryContinueDrag -> CANCEL (ESC pressed)");
            return DRAGDROP_S_CANCEL;
        }

        // Left button is down if either reported by grfkeystate or physically held down
        let lbutton_down = (grfkeystate.0 & MK_LBUTTON) != 0
            || unsafe { (windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x01) as i16) < 0 };

        if !lbutton_down {
            crate::log_msg(format!("QueryContinueDrag -> DROP (button released, grfkeystate={:#x})", grfkeystate.0));
            return DRAGDROP_S_DROP;
        }

        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

#[allow(non_snake_case)]
#[implement(IEnumFORMATETC)]
pub struct ShelfEnumFormatEtc {
    formats: Vec<FORMATETC>,
    index: std::sync::atomic::AtomicUsize,
}

impl ShelfEnumFormatEtc {
    pub fn new(formats: Vec<FORMATETC>) -> Self {
        Self {
            formats,
            index: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl IEnumFORMATETC_Impl for ShelfEnumFormatEtc_Impl {
    fn Next(&self, celt: u32, rgelt: *mut FORMATETC, pceltfetched: *mut u32) -> HRESULT {
        if rgelt.is_null() || (celt > 1 && pceltfetched.is_null()) {
            return E_INVALIDARG;
        }

        let mut fetched = 0;
        let start = self.index.load(std::sync::atomic::Ordering::SeqCst);
        let count = self.formats.len();

        for i in 0..celt as usize {
            if start + i < count {
                unsafe {
                    *rgelt.add(i) = self.formats[start + i];
                }
                fetched += 1;
            } else {
                break;
            }
        }

        self.index.fetch_add(fetched as usize, std::sync::atomic::Ordering::SeqCst);

        if !pceltfetched.is_null() {
            unsafe {
                *pceltfetched = fetched;
            }
        }

        if fetched == celt {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> windows::core::Result<()> {
        let cur = self.index.load(std::sync::atomic::Ordering::SeqCst);
        let next = (cur + celt as usize).min(self.formats.len());
        self.index.store(next, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn Reset(&self) -> windows::core::Result<()> {
        self.index.store(0, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IEnumFORMATETC> {
        let cloned = ShelfEnumFormatEtc {
            formats: self.formats.clone(),
            index: std::sync::atomic::AtomicUsize::new(
                self.index.load(std::sync::atomic::Ordering::SeqCst),
            ),
        };
        Ok(cloned.into())
    }
}

#[allow(non_snake_case)]
#[implement(IDataObject)]
pub struct ShelfDataObject {
    text: Option<String>,
    files: Vec<String>,
}

impl ShelfDataObject {
    pub fn new(text: Option<String>, files: Vec<String>) -> Self {
        let text = text.or_else(|| {
            if !files.is_empty() {
                Some(files.join("\r\n"))
            } else {
                None
            }
        });
        Self { text, files }
    }

    fn create_hdrop(&self) -> Option<HGLOBAL> {
        create_hdrop_from_paths(&self.files)
    }
}

pub fn create_hdrop_from_paths(files: &[String]) -> Option<HGLOBAL> {
    if files.is_empty() {
        return None;
    }

    let mut buffer: Vec<u16> = Vec::new();
    for file in files {
        buffer.extend(file.encode_utf16());
        buffer.push(0);
    }
    buffer.push(0); // double null terminator

    let header_size = size_of::<DROPFILES>();
    let data_size = buffer.len() * size_of::<u16>();
    let total_size = header_size + data_size;

    unsafe {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_size).ok()?;
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            return None;
        }

        let dropfiles = ptr as *mut DROPFILES;
        (*dropfiles).pFiles = header_size as u32;
        (*dropfiles).pt = POINT { x: 0, y: 0 };
        (*dropfiles).fNC = BOOL(0);
        (*dropfiles).fWide = BOOL(1); // Unicode

        let data_ptr = (ptr as *mut u8).add(header_size) as *mut u16;
        copy_nonoverlapping(buffer.as_ptr(), data_ptr, buffer.len());

        let _ = GlobalUnlock(hglobal);
        Some(hglobal)
    }
}

impl ShelfDataObject_Impl {

    fn create_unicode_text(&self) -> Option<HGLOBAL> {
        let text = self.text.as_ref()?;
        let mut buffer: Vec<u16> = text.encode_utf16().collect();
        buffer.push(0);

        let total_size = buffer.len() * size_of::<u16>();

        unsafe {
            let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_size).ok()?;
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                return None;
            }

            copy_nonoverlapping(buffer.as_ptr(), ptr as *mut u16, buffer.len());
            let _ = GlobalUnlock(hglobal);
            Some(hglobal)
        }
    }

    fn create_ansi_text(&self) -> Option<HGLOBAL> {
        let text = self.text.as_ref()?;
        let bytes = text.as_bytes();
        let total_size = bytes.len() + 1; // null-terminated

        unsafe {
            let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_size).ok()?;
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                return None;
            }

            copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
            let _ = GlobalUnlock(hglobal);
            Some(hglobal)
        }
    }
}

#[allow(non_snake_case)]
impl IDataObject_Impl for ShelfDataObject_Impl {
    fn GetData(&self, pformatetcIn: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if pformatetcIn.is_null() {
            return Err(DV_E_FORMATETC.into());
        }

        unsafe {
            let format = &*pformatetcIn;
            let hglobal = match format.cfFormat {
                CF_HDROP => self.create_hdrop(),
                CF_UNICODETEXT => self.create_unicode_text(),
                CF_TEXT => self.create_ansi_text(),
                _ => None,
            };

            if let Some(h) = hglobal {
                let mut medium = STGMEDIUM::default();
                medium.tymed = TYMED_HGLOBAL.0 as u32;
                medium.u.hGlobal = h;
                medium.pUnkForRelease = ManuallyDrop::new(None);
                Ok(medium)
            } else {
                Err(DV_E_FORMATETC.into())
            }
        }
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if pformatetc.is_null() {
            return DV_E_FORMATETC;
        }

        unsafe {
            let format = &*pformatetc;
            if format.tymed != 0 && (format.tymed & (TYMED_HGLOBAL.0 as u32)) == 0 {
                return DV_E_TYMED;
            }

            if format.cfFormat == CF_HDROP && !self.files.is_empty() {
                return S_OK;
            }
            if (format.cfFormat == CF_UNICODETEXT || format.cfFormat == CF_TEXT)
                && self.text.is_some()
            {
                return S_OK;
            }
        }
        DV_E_FORMATETC
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectIn: *const FORMATETC,
        _pformatetcOut: *mut FORMATETC,
    ) -> HRESULT {
        E_NOTIMPL
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        // DATADIR_GET = 1
        if dwdirection != 1 {
            return Err(E_NOTIMPL.into());
        }

        let mut formats = Vec::new();
        if !self.files.is_empty() {
            formats.push(FORMATETC {
                cfFormat: CF_HDROP,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
            });
        }
        if self.text.is_some() {
            formats.push(FORMATETC {
                cfFormat: CF_UNICODETEXT,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
            });
            formats.push(FORMATETC {
                cfFormat: CF_TEXT,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
            });
        }

        let enumerator: IEnumFORMATETC = ShelfEnumFormatEtc::new(formats).into();
        Ok(enumerator)
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: Option<&IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(E_NOTIMPL.into())
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(E_NOTIMPL.into())
    }
}

pub fn initiate_drag_drop(text: Option<String>, files: Vec<String>) -> u32 {
    unsafe {
        let data_object: IDataObject = ShelfDataObject::new(text, files).into();
        let drop_source: IDropSource = ShelfDropSource.into();
        let mut effect = DROPEFFECT_NONE;

        let hr = DoDragDrop(
            &data_object,
            &drop_source,
            DROPEFFECT_COPY | DROPEFFECT_MOVE,
            &mut effect,
        );

        crate::log_msg(format!("--> DoDragDrop completed: hr = {:?}, effect = {:?}", hr, effect));
        effect.0
    }
}
