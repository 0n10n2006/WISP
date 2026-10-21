use crate::windows;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / hide Wisp", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Wisp", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("wisp-tray")
        .tooltip("Wisp")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => windows::toggle_island(app),
            "settings" => windows::open_settings(app, None),
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
                windows::toggle_island(tray.app_handle());
            }
        });
    builder = builder.icon(tray_icon(crate::system_theme::get().system_light));
    builder.build(app)?;
    Ok(())
}

/// Monochrome tray glyph that contrasts with the taskbar.
pub fn tray_icon(light_taskbar: bool) -> Image<'static> {
    let bytes: &'static [u8] = if light_taskbar {
        include_bytes!("../icons/tray-dark-32.png")
    } else {
        include_bytes!("../icons/tray-light-32.png")
    };
    Image::from_bytes(bytes).expect("bundled tray icon is a valid PNG")
}

/// Re-pick the tray glyph after the Windows theme changes.
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("wisp-tray") {
        let _ = tray.set_icon(Some(tray_icon(crate::system_theme::get().system_light)));
    }
}
