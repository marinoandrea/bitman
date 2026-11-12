//! Shared labels and control styling; event handlers are supplied by each screen.

use gpui::{Div, IntoElement, Styled, div, prelude::*, px, relative, rgb};

pub const INK: u32 = 0x252931;
pub const MUTED: u32 = 0x858b94;
pub const LINE: u32 = 0xe1e4e8;
pub const PANEL: u32 = 0xffffff;
pub const ACCENT: u32 = 0x315ed8;
pub const WASH: u32 = 0xf4f5f7;

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
        .child(heading(title).text_sm())
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
        .bg(rgb(PANEL))
        .text_color(rgb(MUTED))
        .text_xs()
        .child(text)
}

pub fn primary_button(text: &'static str) -> Div {
    button(text)
        .bg(rgb(0xe3e9fa))
        .border_color(rgb(0xe3e9fa))
        .text_color(rgb(0x879acb))
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
                .text_xs()
                .text_color(rgb(MUTED))
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
        .text_xs()
        .text_color(rgb(MUTED))
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
                        .bg(rgb(LINE)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .h(px(3.0))
                        .w(relative(fraction))
                        .rounded_full()
                        .bg(rgb(0x9baacb)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .left(relative(fraction))
                        .size(px(11.0))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x9baacb))
                        .bg(rgb(PANEL)),
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
                .border_color(rgb(if checked { 0x9baacb } else { LINE }))
                .bg(rgb(if checked { 0xeaf0ff } else { PANEL }))
                .text_color(rgb(ACCENT))
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
