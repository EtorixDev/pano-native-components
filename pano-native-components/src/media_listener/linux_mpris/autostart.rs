use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use ashpd::desktop::background::Background;

const APP_BINARY: &str = "pano-scrobbler-etd";

fn get_exec_for_autostart() -> String {
    let self_path = std::env::current_exe().and_then(fs::canonicalize).ok();

    let in_path = std::env::var("PATH").ok().and_then(|path_var| {
        path_var
            .split(':')
            .map(|dir| Path::new(dir).join(APP_BINARY))
            .find(|p| p.is_file())
            .and_then(|p| fs::canonicalize(p).ok())
    });

    match (self_path, in_path) {
        (Some(s), Some(p)) if s == p => APP_BINARY.to_string(),
        (Some(s), _) => s.to_string_lossy().into_owned(),
        _ => APP_BINARY.to_string(),
    }
}

pub fn autostart(add: bool) {
    let desktop_file = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })
        .map(|x| x.join("autostart/pano-scrobbler-etd.desktop"));

    let desktop_file = match desktop_file {
        Some(path) => path,
        None => {
            log::error!("Could not determine autostart file path (HOME not set)");
            return;
        }
    };

    if add {
        let exec_path = if let Ok(appimage) = env::var("APPIMAGE") {
            appimage
        } else {
            get_exec_for_autostart()
        };

        // Escape embedded double-quotes before wrapping in quotes per XDG spec
        let escaped = exec_path.replace('"', "\\\"");
        let exec_command = format!("\"{escaped}\" --minimized");

        let contents = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Pano Scrobbler ETD\n\
             Comment=Feature packed music tracker\n\
             Terminal=false\n\
             Exec={exec_command}\n\
             Icon={APP_BINARY}\n\
             X-GNOME-Autostart-enabled=true\n\
             StartupWMClass={APP_BINARY}\n\
             Categories=AudioVideo;Audio;\n"
        );

        if let Some(parent) = desktop_file.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            log::error!("Failed to create autostart directory: {e}");
            return;
        }

        if let Err(e) = fs::write(&desktop_file, contents) {
            log::error!("Failed to write autostart file: {e}");
        }
    } else {
        if let Err(e) = fs::remove_file(&desktop_file) {
            log::error!("Failed to remove autostart file: {e}");
        }
    }
}

pub async fn autostart_sandboxed(add: bool) {
    match Background::request()
        .reason("Start Pano Scrobbler ETD on login")
        .auto_start(add)
        .command([APP_BINARY, "--minimized"])
        .dbus_activatable(false)
        .send()
        .await
    {
        Err(e) => log::error!("Failed to send background autostart request: {e}"),
        Ok(response) => {
            if let Err(e) = response.response() {
                log::error!("Background autostart request denied: {e}");
            }
        }
    }
}
