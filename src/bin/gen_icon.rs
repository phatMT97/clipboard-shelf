use std::fs::File;
use std::io::Write;
use image::{Rgba, RgbaImage, imageops::{resize, FilterType}};

fn dist_to_rounded_rect(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let half_w = ((x1 - x0) / 2.0).max(0.0);
    let half_h = ((y1 - y0) / 2.0).max(0.0);
    let r = r.min(half_w).min(half_h).max(0.0);
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let dx = (px - cx).abs() - (half_w - r);
    let dy = (py - cy).abs() - (half_h - r);

    let ox = dx.max(0.0);
    let oy = dy.max(0.0);
    let out_dist = (ox * ox + oy * oy).sqrt();
    let in_dist = dx.max(dy).min(0.0);

    out_dist + in_dist - r
}

fn blend_pixel(bg: &mut Rgba<u8>, fg: [u8; 4], alpha_factor: f32) {
    let fg_a = (fg[3] as f32 / 255.0) * alpha_factor.clamp(0.0, 1.0);
    if fg_a <= 0.0 {
        return;
    }
    let bg_a = bg[3] as f32 / 255.0;
    let out_a = fg_a + bg_a * (1.0 - fg_a);
    if out_a <= 0.0 {
        return;
    }

    for c in 0..3 {
        let fg_c = fg[c] as f32 / 255.0;
        let bg_c = bg[c] as f32 / 255.0;
        let out_c = (fg_c * fg_a + bg_c * bg_a * (1.0 - fg_a)) / out_a;
        bg[c] = (out_c * 255.0).round() as u8;
    }
    bg[3] = (out_a * 255.0).round() as u8;
}

fn lerp_color(c0: [u8; 4], c1: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        (c0[0] as f32 * (1.0 - t) + c1[0] as f32 * t).round() as u8,
        (c0[1] as f32 * (1.0 - t) + c1[1] as f32 * t).round() as u8,
        (c0[2] as f32 * (1.0 - t) + c1[2] as f32 * t).round() as u8,
        (c0[3] as f32 * (1.0 - t) + c1[3] as f32 * t).round() as u8,
    ]
}

fn draw_rounded_rect_grad(
    img: &mut RgbaImage,
    x0: f32, y0: f32, x1: f32, y1: f32, r: f32,
    fill_top: [u8; 4], fill_bot: [u8; 4],
    border_color: [u8; 4],
    border_width: f32,
) {
    let min_x = (x0 - border_width - 2.0).max(0.0) as u32;
    let max_x = (x1 + border_width + 2.0).min(img.width() as f32) as u32;
    let min_y = (y0 - border_width - 2.0).max(0.0) as u32;
    let max_y = (y1 + border_width + 2.0).min(img.height() as f32) as u32;

    for y in min_y..max_y {
        let t = ((y as f32 + 0.5 - y0) / (y1 - y0)).clamp(0.0, 1.0);
        let current_fill = lerp_color(fill_top, fill_bot, t);

        for x in min_x..max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = dist_to_rounded_rect(px, py, x0, y0, x1, y1, r);

            if border_width > 0.0 {
                if d <= 0.5 && d >= -border_width - 0.5 {
                    let edge_alpha = (0.5 - d).clamp(0.0, 1.0) * (d + border_width + 0.5).clamp(0.0, 1.0);
                    let pixel = img.get_pixel_mut(x, y);
                    blend_pixel(pixel, border_color, edge_alpha);
                }
            }

            if d < -border_width + 0.5 {
                let fill_alpha = (-border_width + 0.5 - d).clamp(0.0, 1.0);
                let pixel = img.get_pixel_mut(x, y);
                blend_pixel(pixel, current_fill, fill_alpha);
            }
        }
    }
}

/// Creates an ultra-clean, modern, minimalist Clipboard Shelf icon (256x256).
fn create_minimal_shelf_icon_256() -> RgbaImage {
    let mut img = RgbaImage::new(256, 256);

    // 1. Subtle smooth ambient shadow for depth
    for i in (1..=8).rev() {
        let extra = i as f32 * 2.2;
        let alpha = (14.0 - i as f32 * 1.5).max(0.0) as u8;
        draw_rounded_rect_grad(
            &mut img,
            38.0 - extra, 36.0 - extra, 218.0 + extra, 240.0 + extra, 28.0 + extra,
            [0, 0, 0, 0], [0, 0, 0, 0],
            [0, 0, 0, alpha],
            extra,
        );
    }

    // 2. Clipboard Base Board
    // Elegant deep indigo-navy gradient with a vibrant cyan border
    draw_rounded_rect_grad(
        &mut img,
        38.0, 36.0, 218.0, 240.0, 28.0,
        [30, 41, 59, 255],        // Slate 800
        [15, 23, 42, 255],        // Slate 900
        [56, 189, 248, 255],      // Sky 400
        3.5,
    );

    // 3. Crisp White Paper Sheet (Centered, high contrast, clean)
    draw_rounded_rect_grad(
        &mut img,
        56.0, 64.0, 200.0, 222.0, 16.0,
        [255, 255, 255, 255],     // Pure crisp white
        [248, 250, 252, 255],     // Slate 50
        [226, 232, 240, 255],     // Slate 200
        1.5,
    );

    // 4. Content lines (Clean, simple, rounded bars)
    // Primary / Title bar (Vibrant Sky/Cyan Blue)
    draw_rounded_rect_grad(
        &mut img,
        76.0, 96.0, 150.0, 114.0, 7.0,
        [14, 165, 233, 255],      // Sky 500
        [2, 132, 199, 255],       // Sky 600
        [0, 0, 0, 0],
        0.0,
    );

    // Body line 1 (Slate 400)
    draw_rounded_rect_grad(
        &mut img,
        76.0, 130.0, 180.0, 144.0, 5.0,
        [148, 163, 184, 255],
        [148, 163, 184, 255],
        [0, 0, 0, 0],
        0.0,
    );

    // Body line 2 (Slate 300)
    draw_rounded_rect_grad(
        &mut img,
        76.0, 158.0, 170.0, 172.0, 5.0,
        [203, 213, 225, 255],
        [203, 213, 225, 255],
        [0, 0, 0, 0],
        0.0,
    );

    // Body line 3 (Slate 200)
    draw_rounded_rect_grad(
        &mut img,
        76.0, 186.0, 136.0, 200.0, 5.0,
        [226, 232, 240, 255],
        [226, 232, 240, 255],
        [0, 0, 0, 0],
        0.0,
    );

    // 5. Minimalist Metallic Top Clip
    // Top Clamp Bar (Clean, rounded pill with sleek metallic gradient)
    draw_rounded_rect_grad(
        &mut img,
        86.0, 24.0, 170.0, 54.0, 10.0,
        [241, 245, 249, 255],     // Slate 100
        [203, 213, 225, 255],     // Slate 300
        [148, 163, 184, 255],     // Slate 400
        2.0,
    );

    // Clip Accent Grip (Vibrant cyan/sky center pill)
    draw_rounded_rect_grad(
        &mut img,
        104.0, 33.0, 152.0, 45.0, 5.0,
        [56, 189, 248, 255],      // Sky 400
        [2, 132, 199, 255],       // Sky 600
        [0, 0, 0, 0],
        0.0,
    );

    img
}

fn write_ico(sizes: &[RgbaImage], out_path: &str) -> std::io::Result<()> {
    let mut file = File::create(out_path)?;

    // 1. ICO Header
    file.write_all(&0u16.to_le_bytes())?; // Reserved
    file.write_all(&1u16.to_le_bytes())?; // Type (1 = ICO)
    file.write_all(&(sizes.len() as u16).to_le_bytes())?; // Number of images

    // Encode each image to PNG in memory
    let mut png_data_list = Vec::new();
    for img in sizes {
        let mut png_buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
            .expect("Failed to encode PNG frame");
        png_data_list.push(png_buf);
    }

    // 2. Directory entries
    let header_size = 6 + (sizes.len() * 16);
    let mut current_offset = header_size as u32;

    for (img, png_data) in sizes.iter().zip(&png_data_list) {
        let w = if img.width() >= 256 { 0u8 } else { img.width() as u8 };
        let h = if img.height() >= 256 { 0u8 } else { img.height() as u8 };

        file.write_all(&[w])?;
        file.write_all(&[h])?;
        file.write_all(&[0u8])?; // Color count
        file.write_all(&[0u8])?; // Reserved
        file.write_all(&1u16.to_le_bytes())?; // Color planes
        file.write_all(&32u16.to_le_bytes())?; // Bits per pixel
        file.write_all(&(png_data.len() as u32).to_le_bytes())?; // Size of PNG data
        file.write_all(&current_offset.to_le_bytes())?; // Image offset

        current_offset += png_data.len() as u32;
    }

    // 3. Image data blocks
    for png_data in &png_data_list {
        file.write_all(png_data)?;
    }

    Ok(())
}

fn main() {
    println!("--> Generating clean, minimalist Clipboard Shelf application icons...");
    let icon_256 = create_minimal_shelf_icon_256();

    // Save master 256x256 PNG
    icon_256.save("assets/app_icon.png").expect("Failed to save assets/app_icon.png");
    println!("--> Saved assets/app_icon.png (256x256)");

    // Generate multi-resolution frames using Lanczos3 downsampling
    let icon_128 = resize(&icon_256, 128, 128, FilterType::Lanczos3);
    let icon_64 = resize(&icon_256, 64, 64, FilterType::Lanczos3);
    let icon_48 = resize(&icon_256, 48, 48, FilterType::Lanczos3);
    let icon_32 = resize(&icon_256, 32, 32, FilterType::Lanczos3);
    let icon_24 = resize(&icon_256, 24, 24, FilterType::Lanczos3);
    let icon_16 = resize(&icon_256, 16, 16, FilterType::Lanczos3);

    // Save standalone 32x32 and 16x16 PNG for UI / tray
    icon_32.save("assets/app_icon_32.png").expect("Failed to save assets/app_icon_32.png");
    icon_16.save("assets/app_icon_16.png").expect("Failed to save assets/app_icon_16.png");
    println!("--> Saved assets/app_icon_32.png and assets/app_icon_16.png");

    // Package all sizes into app_icon.ico (for Windows Taskbar, Explorer, and .exe PE resource)
    let frames = vec![icon_256, icon_128, icon_64, icon_48, icon_32, icon_24, icon_16];
    write_ico(&frames, "assets/app_icon.ico").expect("Failed to write assets/app_icon.ico");
    println!("--> Successfully created assets/app_icon.ico with 7 resolutions (16 to 256px)!");
}
