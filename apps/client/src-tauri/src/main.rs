//! Vardrop — Tauri 2 istemcisi.
//! Ctrl+Alt+V global kısayolu imleç yanında şeffaf arama penceresini açar/kapatır.
//! HTTP istekleri JS tarafındaki fetch ile gider; Rust tarafı ayar dosyası,
//! pencere yönetimi ve kopyalama (arboard) sağlar.
//!
//! Bağlantı bilgisi OS anahtarlığında değil, kullanıcıya özel bir dosyada durur
//! (kurumsal WDAC/Credential Manager sapması). Ayrıntı: SECURITY.md.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs, path::PathBuf, sync::atomic::{AtomicIsize, Ordering}};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Popup açılmadan önce ön plandaki pencere. Paste-after-focus buraya döner.
static TARGET_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Kullanıcıya özel ayar dizini. Windows'ta Roaming değil Local — PAT dosya
/// sunucusuna replike olmasın. Eski `variable-filler` yolu okunursa taşınır.
fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .ok()?;
        return Some(PathBuf::from(base).join("vardrop"));
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        return Some(PathBuf::from(home).join("Library/Application Support/vardrop"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("vardrop"));
            }
        }
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config/vardrop"))
    }
}

/// Eski isimle (`variable-filler`) kurulu dizinler. v0.1 Roaming'e yazdı,
/// sonra Local'e taşındı; ikisi de yenisine kopyalanır.
fn legacy_data_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            out.push(PathBuf::from(base).join("variable-filler"));
        }
    }
    if let Ok(base) = std::env::var("APPDATA") {
        let roaming = PathBuf::from(base).join("variable-filler");
        if !out.contains(&roaming) {
            out.push(roaming);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            out.push(PathBuf::from(home).join("Library/Application Support/variable-filler"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                out.push(PathBuf::from(xdg).join("variable-filler"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            out.push(PathBuf::from(home).join(".config/variable-filler"));
        }
    }
    let current = data_dir();
    out.retain(|p| Some(p) != current.as_ref());
    out
}

fn restrict_dir(p: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    {
        let _ = p;
    }
}

fn restrict_file(p: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = p;
    }
}

fn migrate_legacy() {
    let Some(dir) = data_dir() else {
        return;
    };
    if legacy_data_dirs().iter().all(|legacy| !legacy.is_dir()) {
        return;
    }
    let _ = fs::create_dir_all(&dir);
    restrict_dir(&dir);
    for legacy in legacy_data_dirs() {
        if !legacy.is_dir() {
            continue;
        }
        for name in ["server.txt", "token.txt"] {
            let src = legacy.join(name);
            let dst = dir.join(name);
            if dst.exists() || !src.is_file() {
                continue;
            }
            if fs::copy(&src, &dst).is_ok() {
                restrict_file(&dst);
                let _ = fs::remove_file(&src);
            }
        }
    }
}

fn read_file(p: &PathBuf) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_file(p: &PathBuf, value: &str) -> Result<(), String> {
    fs::write(p, value).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct Settings {
    server: Option<String>,
    token: Option<String>,
    has_token: bool,
    shortcut: String,
}

#[tauri::command]
fn get_settings() -> Settings {
    migrate_legacy();
    let dir = data_dir();
    let server = dir.as_ref().and_then(|d| read_file(&d.join("server.txt")));
    let token = dir.as_ref().and_then(|d| read_file(&d.join("token.txt")));
    Settings {
        server,
        has_token: token.as_ref().map(|t| !t.is_empty()).unwrap_or(false),
        token,
        shortcut: "Ctrl+Alt+V".into(),
    }
}

#[tauri::command]
fn save_settings(server: String, token: String) -> Result<(), String> {
    migrate_legacy();
    let dir = data_dir().ok_or_else(|| "ayar klasörü bulunamadı (HOME/LOCALAPPDATA)".to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    restrict_dir(&dir);
    let server_path = dir.join("server.txt");
    write_file(&server_path, server.trim())?;
    restrict_file(&server_path);

    let tok = token.trim();
    let token_path = dir.join("token.txt");
    if tok.is_empty() {
        let _ = fs::remove_file(&token_path);
    } else {
        write_file(&token_path, tok)?;
        restrict_file(&token_path);
    }
    Ok(())
}

/// Sunucu adresi ve PAT dosyalarını siler (bu makinedeki bağlantı kaydı).
#[tauri::command]
fn clear_settings() -> Result<(), String> {
    let mut dirs = Vec::new();
    if let Some(d) = data_dir() {
        dirs.push(d);
    }
    dirs.extend(legacy_data_dirs());
    for dir in dirs {
        for name in ["server.txt", "token.txt"] {
            let p = dir.join(name);
            if p.exists() {
                fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Seçili değeri panoya kopyalar (JS navigator.clipboard'dan sonra yedek yol).
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text).map_err(|e| e.to_string())
}

/// Görseli panoya resim olarak koyar (base64 kodlu dosya içeriği).
#[tauri::command]
fn copy_image(b64: String) -> Result<(), String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("görsel çözülemedi: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_image(arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    })
    .map_err(|e| e.to_string())
}

/// Popup açılmadan önce ön plandaki pencereyi hatırlar (hedef odak).
fn capture_target_window() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let hwnd = unsafe { GetForegroundWindow() };
        TARGET_WINDOW.store(hwnd as isize, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn send_paste_keys() -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };
    unsafe {
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, 0, 0);
        keybd_event(VK_V as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn send_paste_keys() -> Result<(), String> {
    let status = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to keystroke \"v\" using command down",
        ])
        .status()
        .map_err(|e| format!("osascript çalıştırılamadı: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("osascript başarısız (Erişilebilirlik izni gerekli)".into())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn send_paste_keys() -> Result<(), String> {
    let status = std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .status()
        .map_err(|_| "xdotool bulunamadı — paste için kurun (apt install xdotool)".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("xdotool başarısız".into())
    }
}

/// Popup'ı kapatır, odağı popup açılmadan önceki pencereye verir ve Ctrl+V yollar.
/// Beklemeler bloklayıcı olduğu için spawn_blocking'te çalışır.
#[tauri::command]
async fn paste_to_target(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("search") {
        let _ = win.hide();
    }
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_millis(120));

        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::HWND;
            use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
            let h = TARGET_WINDOW.load(Ordering::SeqCst);
            if h != 0 {
                unsafe {
                    SetForegroundWindow(h as HWND);
                }
                std::thread::sleep(std::time::Duration::from_millis(90));
            }
        }
        #[cfg(target_os = "macos")]
        {
            // popup gizlenince odak önceki uygulamaya döner; kısa bekleme yeterli
            std::thread::sleep(std::time::Duration::from_millis(90));
        }

        send_paste_keys()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tarayıcıda URL açar (opener JS API'si başarısız olursa yedek yol).
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_popup(app: AppHandle) {
    if let Some(win) = app.get_webview_window("search") {
        let _ = win.hide();
    }
}

/// Popup açıkken kapat, kapalıyken imlecin yanına getirip göster (toggle).
fn toggle_popup(app: &AppHandle) {
    let Some(win) = app.get_webview_window("search") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }
    capture_target_window();
    place_near_cursor(&win);
    let _ = win.show();
    let _ = win.set_focus();
}

/// Sistem tepsisi: pencere görünmese de uygulama yaşar, tepside ikon + menü.
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Emitter;

    let search = MenuItem::with_id(app, "search", "Ara (Ctrl+Alt+V)", true, None::<&str>)?;
    let conn = MenuItem::with_id(app, "conn", "Bağlantı…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
    let menu = MenuBuilder::new(app).items(&[&search, &conn]).separator().item(&quit).build()?;

    let tray = TrayIconBuilder::with_id("vardrop-tray")
        .icon(app.default_window_icon().expect("ikon").clone())
        .tooltip("Vardrop — Ctrl+Alt+V")
        .menu(&menu)
        // sol tık menü değil popup açsın
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "search" => toggle_popup(app),
            "conn" => {
                if let Some(win) = app.get_webview_window("search") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                // JS setup ekranını açar (server adresi hatırlanır)
                let _ = app.emit("tray-open-setup", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popup(tray.app_handle());
            }
        })
        .build(app)?;
    let _ = tray;
    Ok(())
}

/// İmlecin hemen yanına koyar; ekran sağ/dibi taşarsa içeri alır.
fn place_near_cursor(win: &WebviewWindow) {
    let app = win.app_handle();
    let Ok(pos) = app.cursor_position() else {
        return;
    };
    let size = win
        .outer_size()
        .unwrap_or_else(|_| PhysicalSize::new(520, 440));
    let mut x = pos.x + 16.0;
    let mut y = pos.y + 16.0;
    if let Ok(Some(mon)) = win.current_monitor() {
        let mon_x = mon.position().x as f64;
        let mon_y = mon.position().y as f64;
        let mon_w = mon.size().width as f64;
        let mon_h = mon.size().height as f64;
        if x + size.width as f64 > mon_x + mon_w {
            x = pos.x - 16.0 - size.width as f64;
        }
        if y + size.height as f64 > mon_y + mon_h {
            y = pos.y - 16.0 - size.height as f64;
        }
        x = x.max(mon_x);
        y = y.max(mon_y);
    }
    let _ = win.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["ctrl+alt+v"])
                .expect("global kayıt başarısız")
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_popup(app);
                    }
                })
                .build(),
        )
        // tepsi ikonu: uygulama penceresiz de yaşar (çıkış = tepsi menüsü)
        .setup(|app| {
            setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            clear_settings,
            copy_text,
            copy_image,
            paste_to_target,
            hide_popup,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("vardrop başlatılamadı");
}
