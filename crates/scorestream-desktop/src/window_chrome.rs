//! Read-only desktop chrome preferences injected before React mounts.
//! No renderer command accepts a setting name, process command, or arbitrary window label.
use serde_json::json;

pub fn initialization_script() -> String {
    let (layout, action, theme) = platform_preferences();
    format!(
        "Object.defineProperty(window, '__SCORESTREAM_WINDOW__', {{ value: Object.freeze({}), writable: false }});",
        json!({
            "platform": std::env::consts::OS,
            "decorationLayout": layout,
            "doubleClickAction": action,
            "theme": theme,
        })
    )
}

#[cfg(target_os = "linux")]
fn platform_preferences() -> (String, &'static str, String) {
    use gtk::prelude::GtkSettingsExt;
    let settings = gtk::Settings::default();
    let layout = settings
        .as_ref()
        .and_then(|settings| settings.gtk_decoration_layout())
        .map(|value| value.to_string())
        .unwrap_or_else(|| ":minimize,maximize,close".into());
    let action = settings
        .as_ref()
        .and_then(|settings| settings.gtk_titlebar_double_click());
    let action = match action.as_deref() {
        Some("minimize") => "minimize",
        Some("none") => "none",
        _ => "maximize",
    };
    let theme = settings
        .and_then(|settings| settings.gtk_theme_name())
        .map(|value| value.to_string())
        .unwrap_or_default();
    (layout, action, theme)
}

#[cfg(target_os = "macos")]
fn platform_preferences() -> (String, &'static str, String) {
    use objc2_foundation::{NSString, NSUserDefaults};
    // AppKit's native traffic lights retain fullscreen/Option-zoom/hover tiling.
    // The custom titlebar double-click is zoom/minimize/none, not fullscreen.
    let defaults = NSUserDefaults::standardUserDefaults();
    let preference = defaults.stringForKey(&NSString::from_str("AppleActionOnDoubleClick"));
    let action = match preference
        .as_ref()
        .map(|value| value.to_string())
        .as_deref()
    {
        Some("Minimize") => "minimize",
        Some("None") => "none",
        Some("Maximize" | "Zoom" | "Fill") => "maximize",
        _ if defaults.boolForKey(&NSString::from_str("AppleMiniaturizeOnDoubleClick")) => {
            "minimize"
        }
        _ => "maximize",
    };
    (String::new(), action, String::new())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_preferences() -> (String, &'static str, String) {
    (":minimize,maximize,close".into(), "maximize", String::new())
}
