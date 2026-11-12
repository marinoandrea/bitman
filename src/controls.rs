//! Shared labels and control styling; event handlers are supplied by each screen.

use gpui::{
    Div, IntoElement, PathBuilder, Pixels, Point, Styled, Window, canvas, div, point, prelude::*,
    px, relative, rgb,
};

// Boot Tool's boottool/static/index.html palette at the revision in README.md.
pub const INK: u32 = 0x000000;
pub const MUTED: u32 = 0x6b6b6b;
pub const LINE: u32 = 0x000000;
pub const PANEL: u32 = 0xdcdcdc;
pub const ACCENT: u32 = 0x404040;
pub const WASH: u32 = 0xd3d3d3;
pub const BUTTON: u32 = 0xefefef;
pub const WHITE: u32 = 0xffffff;

pub fn column() -> Div {
    div().flex().flex_col().gap(px(12.0))
}

pub fn row() -> Div {
    div().flex().items_center().gap(px(8.0))
}

pub fn muted(text: impl Into<gpui::SharedString>) -> Div {
    div().text_color(rgb(MUTED)).text_xs().child(text.into())
}

pub fn label(text: &'static str) -> Div {
    div().text_xs().text_color(rgb(INK)).child(text)
}

pub fn heading(text: &'static str) -> Div {
    div()
        .font_family(crate::fonts::DISPLAY)
        .font_weight(gpui::FontWeight::NORMAL)
        .child(text)
}

pub fn labeled_input(title: &'static str, value: &'static str) -> Div {
    column()
        .flex_1()
        .min_w_0()
        .gap(px(6.0))
        .child(label(title))
        .child(input(value))
}

pub fn section(title: &'static str, content: impl IntoElement) -> Div {
    column()
        .gap(px(14.0))
        .p(px(16.0))
        .border_b_1()
        .border_color(rgb(LINE))
        .child(
            heading(title)
                .text_sm()
                .px(px(12.0))
                .py(px(6.0))
                .bg(rgb(ACCENT))
                .text_color(rgb(WHITE)),
        )
        .child(content)
}

pub fn button(text: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px(px(12.0))
        .h(px(32.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(BUTTON))
        .text_color(rgb(INK))
        .text_xs()
        .child(text)
}

pub fn primary_button(text: &'static str) -> Div {
    button(text).bg(rgb(ACCENT)).text_color(rgb(WHITE))
}

pub fn picker_button(text: &'static str) -> Div {
    button("")
        .gap(px(6.0))
        .child(
            canvas(
                |_, _, _| (),
                |bounds, (), window, _| paint_eyedropper(bounds.origin, window),
            )
            .size(px(14.0))
            .flex_shrink_0(),
        )
        .child(text)
}

/// A compact button glyph; the sampling cursor is supplied natively by GPUI.
fn paint_eyedropper(origin: Point<Pixels>, window: &mut Window) {
    let scale = 14.0 / 26.0;
    let position = |x, y| origin + point(px((x + 1.0) * scale), px((y + 1.0) * scale));
    let contours: &[(&[(f32, f32)], u32)] = &[
        (
            &[
                (2.0, 22.0),
                (3.0, 17.0),
                (13.0, 7.0),
                (17.0, 11.0),
                (7.0, 21.0),
            ],
            WHITE,
        ),
        (
            &[
                (12.0, 6.0),
                (17.0, 1.0),
                (20.0, 1.0),
                (23.0, 4.0),
                (23.0, 7.0),
                (18.0, 12.0),
            ],
            ACCENT,
        ),
        (
            &[(10.0, 6.0), (12.0, 4.0), (20.0, 12.0), (18.0, 14.0)],
            ACCENT,
        ),
    ];
    for &(points, fill) in contours {
        for (width, color) in [(4.0, WHITE), (0.0, fill), (1.5, INK)] {
            let mut path = if width == 0.0 {
                PathBuilder::fill()
            } else {
                PathBuilder::stroke(px(width * scale))
            };
            path.move_to(position(points[0].0, points[0].1));
            for &(x, y) in &points[1..] {
                path.line_to(position(x, y));
            }
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
        }
    }
}

pub fn field(label: &'static str, value: &'static str) -> Div {
    row()
        .justify_between()
        .child(div().text_xs().child(label))
        .child(
            div()
                .min_w(px(88.0))
                .px(px(10.0))
                .py(px(6.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(rgb(LINE))
                .bg(rgb(WHITE))
                .text_xs()
                .text_color(rgb(INK))
                .child(value),
        )
}

pub fn input(value: &'static str) -> Div {
    div()
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(rgb(LINE))
        .bg(rgb(WHITE))
        .text_xs()
        .text_color(rgb(INK))
        .child(value)
}

pub fn slider(label: &'static str, value: impl Into<gpui::SharedString>, fraction: f32) -> Div {
    column()
        .gap(px(6.0))
        .child(
            row()
                .justify_between()
                .text_xs()
                .child(label)
                .child(div().text_color(rgb(MUTED)).child(value.into())),
        )
        .child(
            div()
                .relative()
                .h(px(14.0))
                .child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .h(px(3.0))
                        .w_full()
                        .rounded_full()
                        .bg(rgb(WHITE)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .h(px(3.0))
                        .w(relative(fraction))
                        .rounded_full()
                        .bg(rgb(ACCENT)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .left(relative(fraction))
                        .size(px(11.0))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(LINE))
                        .bg(rgb(ACCENT)),
                ),
        )
}

pub fn check(label: &'static str, checked: bool) -> Div {
    row()
        .text_xs()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(14.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(rgb(LINE))
                .bg(rgb(if checked { ACCENT } else { WHITE }))
                .text_color(rgb(WHITE))
                .child(if checked { "✓" } else { "" }),
        )
        .child(label)
}

pub fn swatches() -> Div {
    row().gap(px(6.0)).children(
        [
            0xffffff, 0x22252b, 0x315ed8, 0xd44a54, 0xe9bc46, 0x49866c, 0xb49bc8,
        ]
        .into_iter()
        .map(|color| {
            div()
                .size(px(24.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(rgb(LINE))
                .bg(rgb(color))
        }),
    )
}

pub fn stat(label: &'static str, value: &'static str) -> Div {
    column()
        .gap(px(5.0))
        .min_w(px(104.0))
        .child(muted(label))
        .child(div().text_sm().child(value))
}

pub fn empty_message(title: &'static str, detail: &'static str) -> Div {
    column()
        .items_center()
        .gap(px(12.0))
        .child(heading(title).text_lg())
        .child(muted(detail))
}
