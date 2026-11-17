use crate::JniCallback;
use crate::tray::OUTGOING_TRAY_EVENT_TX;
use crate::tray::PanoTrayData;
use crate::tray::TRAY_DATA_TX;
use crate::tray::send_tray_click;
use futures_util::StreamExt;
use ksni::{MenuItem, TrayMethods, menu::StandardItem};
use tokio::sync::{mpsc, watch};

const WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";

struct PanoTray {
    pub data: PanoTrayData,
}

impl ksni::Tray for PanoTray {
    fn id(&self) -> String {
        super::TRAY_NAME.to_string()
    }

    fn icon_name(&self) -> String {
        self.data.icon_name.clone()
    }

    fn icon_theme_path(&self) -> String {
        self.data.icons_dir.clone()
    }

    const MENU_ON_ACTIVATE: bool = true;

    fn title(&self) -> String {
        "Pano Scrobbler ETD".to_string()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        // title is the first line of the tooltip, description is the rest.
        // if the tooltip is only one line, the description will be empty
        // also handle the case where the tooltip is empty

        let mut lines = self.data.tooltip.trim().lines();
        let title = lines.next().unwrap_or("").to_string();
        let description = lines.collect::<Vec<&str>>().join("\n");

        ksni::ToolTip {
            title,
            description,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        self.data
            .menu_items
            .iter()
            .map(|(id, text)| {
                let id_owned = id.clone();
                let text_owned = text.replace("_", "__"); // see ksni docs for why
                match id.as_str() {
                    "Separator" => MenuItem::Separator,
                    _ => MenuItem::Standard(StandardItem {
                        label: text_owned,
                        activate: Box::new(move |_tray| {
                            send_tray_click(id_owned.clone());
                        }),
                        enabled: !id.ends_with("Disabled"),
                        ..Default::default()
                    }),
                }
            })
            .collect()
    }
}

pub async fn tray_listener(
    callback_sender: mpsc::Sender<JniCallback>,
) -> Result<(), Box<dyn std::error::Error>> {
    OUTGOING_TRAY_EVENT_TX.set(callback_sender).unwrap();

    listen_for_tray_updates(TRAY_DATA_TX.subscribe(), ashpd::is_sandboxed()).await;
    Ok(())
}

async fn watch_tray_host() -> zbus::Result<zbus::fdo::NameOwnerChangedStream> {
    let connection = zbus::Connection::session().await?;
    zbus::fdo::DBusProxy::new(&connection)
        .await?
        .receive_name_owner_changed_with_args(&[(0, WATCHER_NAME)])
        .await
}

async fn listen_for_tray_updates(
    mut receiver: watch::Receiver<Option<PanoTrayData>>,
    disable_dbus_name: bool,
) {
    let mut tray_handle: Option<ksni::Handle<PanoTray>> = None;
    let mut watcher_changes = None;

    loop {
        let tray_data = receiver.borrow_and_update().clone();

        if let Some(tray_data) = tray_data {
            // Subscribe before spawning. ksni subscribes after its first registration,
            // so a watcher starting during that gap can otherwise leave a live,
            // unregistered handle behind.
            if watcher_changes.is_none() {
                match watch_tray_host().await {
                    Ok(changes) => watcher_changes = Some(changes),
                    Err(e) => log::error!("Failed to watch tray host: {e}"),
                }
            }

            if let Some(handle) = &tray_handle {
                if handle
                    .update(|existing_tray| {
                        existing_tray.data = tray_data.clone();
                    })
                    .await
                    .is_none()
                {
                    tray_handle = None;
                }
            }

            if tray_handle.is_none() {
                let tray = PanoTray { data: tray_data };
                match tray
                    .disable_dbus_name(disable_dbus_name)
                    // The tray host may come up after us when autostarted at login.
                    .assume_sni_available(true)
                    .spawn()
                    .await
                {
                    Ok(handle) => tray_handle = Some(handle),
                    Err(e) => log::error!("Failed to spawn tray: {e}"),
                }
            }
        }

        tokio::select! {
            event = async {
                match &mut watcher_changes {
                    Some(changes) => changes.next().await,
                    None => std::future::pending().await,
                }
            } => {
                match event {
                    Some(event) => {
                        if event.args().is_ok_and(|args| args.new_owner().is_some()) {
                            // Close the old service before registering its replacement.
                            // The next iteration uses the latest tray data, even if no
                            // update arrived after the watcher started.
                            if let Some(handle) = tray_handle.take() {
                                handle.shutdown().await;
                            }
                        }
                    }
                    None => watcher_changes = None,
                }
            }
            changed = receiver.changed() => {
                if changed.is_err() {
                    break;
                }
            }
        }
    }

    if let Some(handle) = tray_handle {
        handle.shutdown().await;
    }
}

#[cfg(test)]
#[path = "tray_linux_tests.rs"]
mod tests;
