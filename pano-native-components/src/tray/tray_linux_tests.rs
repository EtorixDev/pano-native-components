use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_INTERFACE: &str = "org.kde.StatusNotifierItem";

struct TestProcess(Child);

impl Drop for TestProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn on_private_bus(name: &str, scenario: impl Future<Output = ()>) {
    if let Ok(address) = std::env::var("PANO_TRAY_TEST_BUS") {
        assert_eq!(std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(), address);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(10), scenario)
                    .await
                    .expect("private-bus tray scenario timed out");
            });
        return;
    }

    let mut bus = TestProcess(
        Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("tray tests require dbus-daemon"),
    );
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim();
    assert!(address.starts_with("unix:"));

    // A separate process supplies the session address before any threads start.
    // The parent test process never changes or connects to the desktop session.
    let mut test = TestProcess(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("{}::{name}", module_path!().split_once("::").unwrap().1),
                "--nocapture",
            ])
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env("PANO_TRAY_TEST_BUS", address)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = test.0.try_wait().unwrap() {
            assert!(status.success(), "private-bus tray test failed: {status}");
            break;
        }
        assert!(Instant::now() < deadline, "private-bus child test hung");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Watcher {
    registrations: mpsc::UnboundedSender<Result<String, String>>,
    restart_before_reply: AtomicBool,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: &str,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        if self.restart_before_reply.swap(false, Ordering::SeqCst) {
            // Force the watcher to appear before ksni can process ServiceUnknown
            // and subscribe. No scheduling delay or cached-source patch is needed.
            connection.release_name(WATCHER_NAME).await?;
            connection.request_name(WATCHER_NAME).await?;
            self.registrations.send(Err(service.to_owned())).unwrap();
            return Err(zbus::fdo::Error::ServiceUnknown(
                "Simulated watcher startup during registration.".into(),
            ));
        }
        self.registrations.send(Ok(service.to_owned())).unwrap();
        Ok(())
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
}

async fn watcher(
    restart_before_reply: bool,
) -> (
    zbus::Connection,
    mpsc::UnboundedReceiver<Result<String, String>>,
) {
    let (registrations, receiver) = mpsc::unbounded_channel();
    let connection = zbus::connection::Builder::session()
        .unwrap()
        .serve_at(
            "/StatusNotifierWatcher",
            Watcher {
                registrations,
                restart_before_reply: AtomicBool::new(restart_before_reply),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    (connection, receiver)
}

fn tray_data(icon: &str) -> PanoTrayData {
    PanoTrayData {
        tooltip: "Pano Scrobbler ETD\nPrivate Test".into(),
        icons_dir: "/private-test/icons".into(),
        icon_name: icon.into(),
        menu_items: vec![("ShowApp".into(), "Show App".into())],
    }
}

async fn assert_item(connection: &zbus::Connection, service: &str, icon: &str) {
    let item = zbus::Proxy::new(connection, service, ITEM_PATH, ITEM_INTERFACE)
        .await
        .unwrap();
    assert_eq!(
        item.get_property::<String>("Id").await.unwrap(),
        crate::tray::TRAY_NAME
    );
    assert_eq!(
        item.get_property::<String>("Title").await.unwrap(),
        "Pano Scrobbler ETD"
    );
    assert_eq!(item.get_property::<String>("IconName").await.unwrap(), icon);
}

#[test]
fn registration_race_recovers_without_an_update() {
    on_private_bus("registration_race_recovers_without_an_update", async {
        // Exercise both regular and sandbox-style D-Bus names.
        for disable_dbus_name in [false, true] {
            let (watcher, mut registrations) = watcher(true).await;
            watcher.request_name(WATCHER_NAME).await.unwrap();
            let (data, receiver) = watch::channel(Some(tray_data("initial-icon")));
            let listener = tokio::spawn(listen_for_tray_updates(receiver, disable_dbus_name));

            let failed_service = registrations.recv().await.unwrap().unwrap_err();
            let service = registrations.recv().await.unwrap().unwrap();
            assert_ne!(service, failed_service);
            assert_eq!(service.starts_with(':'), disable_dbus_name);
            assert_item(&watcher, &service, "initial-icon").await;

            drop(data);
            listener.await.unwrap();
            let dbus = zbus::fdo::DBusProxy::new(&watcher).await.unwrap();
            assert!(
                dbus.get_name_owner(service.as_str().try_into().unwrap())
                    .await
                    .is_err()
            );
            watcher.close().await.unwrap();
        }
    });
}

#[test]
fn late_watcher_and_restart_preserve_latest_data() {
    on_private_bus("late_watcher_and_restart_preserve_latest_data", async {
        let (watcher, mut registrations) = watcher(false).await;
        let dbus = zbus::fdo::DBusProxy::new(&watcher).await.unwrap();
        let mut owners = dbus.receive_name_owner_changed().await.unwrap();
        let (data, receiver) = watch::channel(Some(tray_data("initial-icon")));
        let listener = tokio::spawn(listen_for_tray_updates(receiver, false));

        let initial_service = loop {
            let event = owners.next().await.unwrap();
            let args = event.args().unwrap();
            if args
                .name()
                .as_str()
                .starts_with("org.kde.StatusNotifierItem-")
                && args.new_owner().is_some()
            {
                break args.name().to_string();
            }
        };
        let item = zbus::Proxy::new(
            &watcher,
            initial_service.as_str(),
            ITEM_PATH,
            ITEM_INTERFACE,
        )
        .await
        .unwrap();
        let mut icons = item.receive_signal("NewIcon").await.unwrap();
        data.send_replace(Some(tray_data("latest-icon")));
        icons.next().await.unwrap();
        assert_item(&watcher, &initial_service, "latest-icon").await;

        watcher.request_name(WATCHER_NAME).await.unwrap();
        let first_service = loop {
            let service = registrations.recv().await.unwrap().unwrap();
            if service != initial_service {
                break service;
            }
        };
        assert_item(&watcher, &first_service, "latest-icon").await;
        assert!(
            dbus.get_name_owner(initial_service.as_str().try_into().unwrap())
                .await
                .is_err()
        );

        watcher.release_name(WATCHER_NAME).await.unwrap();
        watcher.request_name(WATCHER_NAME).await.unwrap();
        let replacement = loop {
            let service = registrations.recv().await.unwrap().unwrap();
            if service != first_service {
                break service;
            }
        };
        assert_item(&watcher, &replacement, "latest-icon").await;
        assert!(
            dbus.get_name_owner(first_service.as_str().try_into().unwrap())
                .await
                .is_err()
        );
        drop(data);
        listener.await.unwrap();
        assert!(
            dbus.get_name_owner(replacement.as_str().try_into().unwrap())
                .await
                .is_err()
        );
    });
}
