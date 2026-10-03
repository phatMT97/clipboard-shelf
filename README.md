<div align="center">

<img src="assets/app_icon.png" width="128" height="128" alt="Clipboard Shelf Icon" />

# 📋 Clipboard Shelf

**A modern, lightweight, high-performance Clipboard Companion for Windows 11.**  
*Xây dựng bằng Rust, Slint UI và Windows Win32 / OLE Drag-and-Drop native.*

[![Release](https://img.shields.io/github/v/release/phatMT97/clipboard-shelf?style=flat-square&color=0284c7)](https://github.com/phatMT97/clipboard-shelf/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-blue?style=flat-square&logo=windows)](https://www.microsoft.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-DEA584?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![UI Framework](https://img.shields.io/badge/UI-Slint%201.8-purple?style=flat-square)](https://slint.dev)

[**Tiếng Việt**](#tính-năng-nổi-bật) | [**English**](#key-features)

</div>

---

## 🇻🇳 Tiếng Việt

**Clipboard Shelf** là ứng dụng khay nhớ tạm hiện đại được thiết kế riêng cho người dùng Windows, giúp quản lý lịch sử copy, hình ảnh chụp màn hình và tệp tin một cách trực quan, tiện lợi và tiết kiệm tài nguyên tối đa.

### ✨ Tính năng nổi bật

1. **Tự động bắt dữ liệu thời gian thực (Background Clipboard Listener)**
   - Lắng nghe sự kiện `WM_CLIPBOARDUPDATE` từ Windows API.
   - Hỗ trợ đầy đủ: **Văn bản** (Unicode & ANSI), **Hình ảnh** (chụp màn hình tự động chuyển sang PNG hiển thị thumbnail tức thì), **Tệp tin & Thư mục** (CF_HDROP).

2. **Kéo thả OLE 2 chiều (True Win32 OLE Drag & Drop)**
   - Kéo trực tiếp bất kỳ tệp tin, hình ảnh hoặc đoạn văn bản nào từ Clipboard Shelf thả sang các phần mềm khác (VS Code, Explorer, Word, Telegram, trình duyệt, v.v.).
   - Hiển thị dải banner chỉ dẫn OLE mượt mà trong quá trình kéo.

3. **Tự động ẩn vào cạnh màn hình (Smart Edge Docking & Auto-Hide)**
   - Cửa sổ tự động gắn chặt vào mép phải màn hình làm việc (Work Area).
   - Tùy chọn **Auto-Hide**: Cửa sổ thu nhỏ thành thanh tab gọn 14px ở cạnh màn hình khi không tương tác và tự động trượt ra mượt mà khi rê chuột đến.

4. **Ghim thẻ quan trọng (Pin to Top)**
   - Nhấn icon đinh ghim để giữ các nội dung quan trọng luôn nằm trên cùng danh sách.
   - Không bị xóa khi thực hiện thao tác dọn dẹp danh sách.

5. **Bộ lọc danh mục & Tìm kiếm tức thì**
   - Lọc nhanh theo các tab: *Tất cả*, *Văn bản*, *Hình ảnh*, *Tệp*.
   - Ô tìm kiếm thời gian thực lọc nhanh theo tiêu đề hoặc nội dung.

6. **Hỗ trợ Song ngữ (VI / EN)**
   - Nút chuyển đổi nhanh `[ VI | EN ]` ngay trên thanh tiêu đề.
   - Tự động dịch tiêu đề thẻ, thông báo và menu khay hệ thống.
   - Ghi nhớ tùy chọn ngôn ngữ cho các lần khởi động tiếp theo.

7. **Tích hợp khay hệ thống (System Tray)**
   - Icon tùy chỉnh sắc nét ở khay hệ thống.
   - Chuột phải mở menu điều khiển: Hiện/Ẩn, Bật/Tắt Tự động ẩn, Xóa thẻ chưa ghim, Đổi ngôn ngữ, Thoát ứng dụng.

8. **Giao diện hiện đại & Icon Fluent tối giản**
   - Thiết kế chuẩn Windows 11 Fluent Design, bo góc mượt mà, theme tối sang trọng.
   - File `.exe`, Taskbar và Khay hệ thống đều đồng bộ bộ icon nhận diện thương hiệu.

---

## 🇬🇧 English

**Clipboard Shelf** is a modern, ultra-fast clipboard companion built with Rust and Slint, providing seamless clipboard history tracking, image thumbnail previewing, and native OLE drag-and-drop.

### 🚀 Key Features

* **Real-time Clipboard Capture:** Listens to Win32 `WM_CLIPBOARDUPDATE` to capture text, screenshots (PNG with instant previews), and files/folders.
* **True Native OLE Drag & Drop:** Drag cards directly into File Explorer, code editors, chat apps, or office suites.
* **Screen Edge Docking & Auto-Hide:** Docks to the right screen edge. When Auto-Hide is enabled, collapses into a 14px subtle edge tab and smoothly slides open on mouse hover.
* **Pinning & Priority Ordering:** Pinned cards always stay at the top and are protected from bulk deletion.
* **Search & Category Filters:** Instantly search through text and filter by *All*, *Text*, *Images*, and *Files*.
* **Dual Language (VI / EN):** One-click toggle between Vietnamese and English, dynamically updating UI strings and tray menus.
* **System Tray Companion:** Full right-click context menu and background resident capability.
* **Minimalist Fluent Design:** High-contrast, clean icons embedded into the PE header, taskbar, and notification tray.

---

## 🛠️ Cài đặt & Hướng dẫn Build (Building from Source)

### Yêu cầu tiên quyết (Prerequisites)
* Hệ điều hành: **Windows 10 / Windows 11 (64-bit)**
* **Rust toolchain** (khuyến nghị phiên bản 1.75 trở lên): [rustup.rs](https://rustup.rs)
* **MSVC Build Tools** (Visual Studio C++ build tools hoặc Windows SDK)

### Các bước biên dịch (Build steps)

1. Clone repository:
   ```bash
   git clone https://github.com/phatMT97/clipboard-shelf.git
   cd clipboard-shelf
   ```

2. Tạo bộ icon (nếu cần cập nhật lại icon):
   ```bash
   cargo run --bin gen_icon
   ```

3. Biên dịch bản Release tối ưu hóa:
   ```bash
   cargo build --release
   ```

4. File thực thi hoàn chỉnh nằm tại:
   ```
   target/release/clipboard-shelf.exe
   ```

---

## ⌨️ Phím tắt & Thao tác chuột (Controls)

| Thao tác | Mô tả chức năng |
| :--- | :--- |
| **Click vào thẻ** | Sao chép lại nội dung vào Clipboard (hiển thị hiệu ứng flash xanh) |
| **Kéo chuột trái (Drag)** | Kích hoạt OLE Drag & Drop để kéo file/text sang ứng dụng khác |
| **Click nút Ghim 📌** | Ghim thẻ lên đầu danh sách / Bỏ ghim |
| **Click nút Xóa 🗑️** | Xóa thẻ khỏi danh sách |
| **Click nút Mở rộng 📄** | Xem toàn bộ nội dung văn bản dài |
| **Click `[ VI \| EN ]`** | Đổi ngôn ngữ giao diện tức thì |
| **Rê chuột vào mép phải** | Cửa sổ tự động trượt ra (khi đang ở chế độ Auto-Hide) |
| **Chuột phải icon Khay (Tray)** | Mở menu cài đặt và điều khiển nhanh |

---

## 📦 Kiến trúc mã nguồn (Architecture)

```
clipboard-shelf/
├── .github/workflows/
│   └── release.yml          # GitHub Actions workflow build release tự động
├── assets/
│   ├── app_icon.png         # Master PNG 256x256
│   ├── app_icon.ico         # Icon Windows 7 resolutions (16px -> 256px)
│   ├── app_icon_32.png      # 32x32 UI icon
│   └── app_icon_16.png      # 16x16 Tray icon
├── src/
│   ├── bin/
│   │   └── gen_icon.rs      # Tool sinh icon SDF độ phân giải cao
│   ├── clipboard.rs         # Win32 clipboard listener & bộ giải mã định dạng
│   ├── domain.rs            # Structs dữ liệu ClipboardItem, ItemType
│   ├── main.rs              # Điều phối sự kiện Slint, logic dock và animation
│   ├── transfer.rs          # Win32 IDropSource & IDataObject (OLE Drag-and-Drop)
│   └── tray.rs              # Win32 Shell_NotifyIcon & menu chuột phải khay hệ thống
├── ui/
│   └── shelf.slint          # Khai báo giao diện người dùng Slint UI
├── app.rc                   # Windows PE Resource Script gắn icon vào file .exe
├── build.rs                 # Script build biên dịch app.rc qua rc.exe và compile Slint
└── Cargo.toml               # Cấu hình dependencies (slint, windows, image,...)
```

---

## 📄 Bản quyền (License)

Dự án được phân phối dưới giấy phép **MIT License**.
