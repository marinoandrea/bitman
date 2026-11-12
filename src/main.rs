mod app;
mod controls;
mod screens;

use app::Workspace;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    Application::new().run(|cx: &mut App| {
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
                cx.new(|_| Workspace {
                    shirt_size: 3,
                    ..Workspace::default()
                })
            },
        )
        .expect("could not open the Bitman window");
        cx.activate(true);
    });
}
