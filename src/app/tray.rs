use std::path::Path;
use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

use iced::Subscription;
use iced::advanced::subscription as iced_subscription;
use iced::advanced::subscription::Hasher;
use iced::futures::stream;
use tokio::time::sleep;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
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
    open_id: MenuId,
    quit_id: MenuId,
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

        Ok(Self {
            _tray_icon: tray_icon,
            _menu: menu,
            _open_item: open_item,
            _quit_item: quit_item,
            open_id,
            quit_id,
        })
    }

    pub fn subscription(&self) -> Subscription<Event> {
        iced_subscription::from_recipe(TraySubscription::new(
            self.open_id.clone(),
            self.quit_id.clone(),
        ))
    }
}

fn build_tray_icon(menu: Menu) -> Result<TrayIcon, String> {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        TrayIconBuilder::new()
            .with_tooltip("PostureTracker")
            .with_icon(app_icon())
            .with_menu(Box::new(menu))
            .build()
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
    open_id: MenuId,
    quit_id: MenuId,
}

impl TraySubscription {
    fn new(open_id: MenuId, quit_id: MenuId) -> Self {
        Self { open_id, quit_id }
    }
}

impl iced_subscription::Recipe for TraySubscription {
    type Output = Event;

    fn hash(&self, state: &mut Hasher) {
        use std::hash::Hash;
        std::any::TypeId::of::<Self>().hash(state);
        self.open_id.hash(state);
        self.quit_id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: stream::BoxStream<iced_subscription::Event>,
    ) -> stream::BoxStream<Self::Output> {
        let open_id = self.open_id;
        let quit_id = self.quit_id;

        let s = async_stream::stream! {
            loop {
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id == open_id {
                        yield Event::OpenRequested;
                    } else if event.id == quit_id {
                        yield Event::QuitRequested;
                    }
                }

                sleep(Duration::from_millis(100)).await;
            }
        };

        Box::pin(s)
    }
}

fn app_icon() -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0_u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let border = x < 3 || x >= width - 3 || y < 3 || y >= height - 3;
            let accent = (12..20).contains(&x) || (12..20).contains(&y);

            let (r, g, b, a) = if border {
                (21, 101, 192, 255)
            } else if accent {
                (95, 181, 255, 255)
            } else {
                (9, 23, 38, 255)
            };

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = a;
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32)
        .expect("tray icon pixels should be valid")
}
