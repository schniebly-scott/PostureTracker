use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, Mutex};

use iced::Subscription;
use iced::advanced::subscription as iced_subscription;
use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone)]
pub enum Event {
    OpenRequested,
    QuitRequested,
}

pub struct TrayState {
    _tray_icon: TrayIcon,
    _menu: Menu,
    _open_item: MenuItem,
    _quit_item: MenuItem,
    /// Receiver for menu activations pushed by the global event handler. Wrapped
    /// so the (re-created-per-render) subscription recipe can take ownership the
    /// first time its stream runs; see `TraySubscription::stream`.
    event_rx: Arc<Mutex<Option<UnboundedReceiver<Event>>>>,
}

impl TrayState {
    pub fn new() -> Result<Self, String> {
        ensure_tray_runtime_available()?;

        let menu = Menu::new();
        let open_item = MenuItem::new("Show PostureTracker", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        menu.append(&open_item)
            .map_err(|error| format!("unable to add open tray menu item: {error}"))?;
        menu.append(&quit_item)
            .map_err(|error| format!("unable to add quit tray menu item: {error}"))?;

        let open_id = open_item.id().clone();
        let quit_id = quit_item.id().clone();

        let tray_icon = build_tray_icon(menu.clone())?;

        // Forward menu activations straight into a channel instead of polling
        // muda's event receiver. The handler is process-global and backed by a
        // `OnceCell` that only honours its first `Some`, so this must run exactly
        // once — `TrayState::new` is constructed a single time at startup.
        let (tx, rx) = mpsc::unbounded_channel();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let mapped = if event.id == open_id {
                Some(Event::OpenRequested)
            } else if event.id == quit_id {
                Some(Event::QuitRequested)
            } else {
                None
            };
            if let Some(event) = mapped {
                // The receiver lives for the app's lifetime; a send error only
                // means we're shutting down, so dropping the event is fine.
                let _ = tx.send(event);
            }
        }));

        Ok(Self {
            _tray_icon: tray_icon,
            _menu: menu,
            _open_item: open_item,
            _quit_item: quit_item,
            event_rx: Arc::new(Mutex::new(Some(rx))),
        })
    }

    pub fn subscription(&self) -> Subscription<Event> {
        iced_subscription::from_recipe(TraySubscription::new(self.event_rx.clone()))
    }
}

fn build_tray_icon(menu: Menu) -> Result<TrayIcon, String> {
    let icon = app_icon()?;
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let builder = TrayIconBuilder::new()
            .with_tooltip("PostureTracker")
            .with_icon(icon)
            .with_menu(Box::new(menu));

        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);

        builder.build()
    }));

    panic::set_hook(previous_hook);

    match result {
        Ok(Ok(tray_icon)) => Ok(tray_icon),
        Ok(Err(error)) => Err(format!("unable to build tray icon: {error}")),
        Err(_) => Err("system tray is unavailable on this Linux environment".to_string()),
    }
}

#[cfg(target_os = "linux")]
fn ensure_tray_runtime_available() -> Result<(), String> {
    let lib_dirs = [
        "/usr/lib",
        "/usr/lib64",
        "/lib",
        "/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/lib/x86_64-linux-gnu",
    ];
    let lib_names = [
        "libayatana-appindicator3.so.1",
        "libappindicator3.so.1",
        "libayatana-appindicator3.so",
        "libappindicator3.so",
    ];

    let installed = lib_dirs.iter().any(|dir| {
        lib_names
            .iter()
            .any(|name| Path::new(dir).join(name).exists())
    });

    if installed {
        Ok(())
    } else {
        Err(
            "system tray disabled: install libayatana-appindicator3 or libappindicator3 to enable it"
                .to_string(),
        )
    }
}

#[cfg(not(target_os = "linux"))]
fn ensure_tray_runtime_available() -> Result<(), String> {
    Ok(())
}

struct TraySubscription {
    event_rx: Arc<Mutex<Option<UnboundedReceiver<Event>>>>,
}

impl TraySubscription {
    fn new(event_rx: Arc<Mutex<Option<UnboundedReceiver<Event>>>>) -> Self {
        Self { event_rx }
    }
}

impl iced_subscription::Recipe for TraySubscription {
    type Output = Event;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        // The recipe hashes to a constant, so iced keeps a single stream alive
        // for the app's lifetime and only polls this first instance. Take the
        // receiver so that long-lived stream owns it; any duplicate recipe finds
        // `None` and ends immediately rather than competing for events.
        let rx = self.event_rx.lock().unwrap().take();

        let s = async_stream::stream! {
            if let Some(mut rx) = rx {
                while let Some(event) = rx.recv().await {
                    yield event;
                }
            }
        };

        Box::pin(s)
    }
}

#[cfg(target_os = "macos")]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/icons/tray/tray-icon-template-24.png");

#[cfg(target_os = "windows")]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/icons/tray/tray-icon-16.png");

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/icons/tray/tray-icon-48.png");

fn app_icon() -> Result<Icon, String> {
    let image = image::load_from_memory_with_format(TRAY_ICON_PNG, image::ImageFormat::Png)
        .map_err(|error| format!("unable to decode embedded tray icon: {error}"))?
        .into_rgba8();
    let (width, height) = image.dimensions();

    Icon::from_rgba(image.into_raw(), width, height)
        .map_err(|error| format!("unable to create tray icon: {error}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_icon_assets_have_expected_dimensions_and_transparency() {
        let assets = [
            (
                include_bytes!("../../assets/icons/tray/tray-icon-16.png").as_slice(),
                16,
            ),
            (
                include_bytes!("../../assets/icons/tray/tray-icon-24.png").as_slice(),
                24,
            ),
            (
                include_bytes!("../../assets/icons/tray/tray-icon-32.png").as_slice(),
                32,
            ),
            (
                include_bytes!("../../assets/icons/tray/tray-icon-48.png").as_slice(),
                48,
            ),
            (
                include_bytes!("../../assets/icons/tray/tray-icon-template-24.png").as_slice(),
                24,
            ),
        ];

        for (png, expected_size) in assets {
            let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
                .expect("embedded tray icon should be a valid PNG")
                .into_rgba8();

            assert_eq!(image.dimensions(), (expected_size, expected_size));
            assert!(
                image.pixels().any(|pixel| pixel.0[3] == 0),
                "tray icon should have a transparent background"
            );
            assert!(
                image.pixels().any(|pixel| pixel.0[3] > 0),
                "tray icon should contain a visible mark"
            );
        }
    }
}
