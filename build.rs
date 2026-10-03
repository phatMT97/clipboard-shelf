fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut rc_candidates = vec![
            "rc.exe".to_string(),
            "C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.26100.0\\x64\\rc.exe".to_string(),
            "C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.22621.0\\x64\\rc.exe".to_string(),
            "C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.19041.0\\x64\\rc.exe".to_string(),
        ];
        if let Ok(entries) = std::fs::read_dir("C:\\Program Files (x86)\\Windows Kits\\10\\bin") {
            for entry in entries.flatten() {
                let candidate = entry.path().join("x64").join("rc.exe");
                if candidate.exists() {
                    rc_candidates.push(candidate.to_string_lossy().to_string());
                }
            }
        }

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let res_path = format!("{}\\app.res", out_dir);
        for rc in &rc_candidates {
            if let Ok(status) = std::process::Command::new(rc)
                .args(&["/fo", &res_path, "app.rc"])
                .status()
            {
                if status.success() {
                    println!("cargo:rustc-link-arg={}", res_path);
                    println!("cargo:rerun-if-changed=app.rc");
                    println!("cargo:rerun-if-changed=assets/app_icon.ico");
                    break;
                }
            }
        }
    }

    slint_build::compile("ui/shelf.slint").unwrap();
}
