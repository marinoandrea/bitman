mod app;
mod controls;
mod editor;
mod effect_ui;
mod fonts;
mod menus;
mod screens;

use app::Workspace;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("bitman {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if std::env::args().any(|argument| argument == "--font-licenses") {
        println!("{}", fonts::LICENSES);
        return;
    }

    Application::new().run(|cx: &mut App| {
        fonts::load(cx);

        menus::init(cx);

        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1440.0), px(960.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1060.0), px(700.0))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Bitman");
                cx.new(|cx| {
                    let mut workspace = Workspace::new(cx);
                    window.focus(&workspace.focus_handle);
                    let mut args = std::env::args().skip(1);
                    while let Some(arg) = args.next() {
                        if arg == "--print" {
                            workspace.screen = app::Screen::Print;
                        }
                        if arg == "--open"
                            && let Some(path) = args.next()
                        {
                            workspace.load_path(path.into(), cx);
                        }
                    }
                    workspace
                })
            },
        )
        .expect("could not open the Bitman window");
        cx.activate(true);
    });
}
