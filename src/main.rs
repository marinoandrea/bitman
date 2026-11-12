mod app;
mod controls;
mod editor;
mod effect_ui;
mod fonts;
mod screens;

use app::Workspace;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds, WindowOptions,
    actions, px, size,
};

actions!(bitman, [Quit, CloseWindow]);

fn main() {
    if std::env::args().any(|argument| argument == "--font-licenses") {
        println!("{}", fonts::LICENSES);
        return;
    }

    Application::new().run(|cx: &mut App| {
        fonts::load(cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CloseWindow, cx| {
            if let Some(window) = cx.active_window() {
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
        });
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
        ]);
        cx.set_menus(vec![
            Menu {
                name: "Bitman".into(),
                items: vec![MenuItem::action("Quit Bitman", Quit)],
            },
            Menu {
                name: "File".into(),
                items: vec![MenuItem::action("Close Window", CloseWindow)],
            },
        ]);

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
                    let mut workspace = Workspace {
                        shirt_size: 3,
                        ..Workspace::default()
                    };
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
