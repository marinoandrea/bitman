use gpui::{Context, Div, IntoElement, Render, Styled, Window, div, prelude::*, px, rgb};

use crate::controls::*;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Convert,
    Compose,
    Print,
    Simulate,
    Help,
}

impl Screen {
    const ALL: [(Self, &'static str); 5] = [
        (Self::Convert, "Convert"),
        (Self::Compose, "Compose"),
        (Self::Print, "Print"),
        (Self::Simulate, "Simulate"),
        (Self::Help, "Help"),
    ];
}

#[derive(Default)]
pub struct Workspace {
    pub editor: crate::editor::Editor,
    pub screen: Screen,
    pub preview: usize,
    pub background: usize,
    pub convert_mode: usize,
    pub upscale_style: usize,
    pub print_method: usize,
    pub print_kind: usize,
    pub print_render: usize,
    pub plate: usize,
    pub print_shape: usize,
    pub dither: usize,
    pub compose_tool: usize,
    pub library: usize,
    pub shirt_side: usize,
    pub shirt_size: usize,
    pub sleeve: usize,
}

impl Workspace {
    pub fn choices(
        &self,
        id: &'static str,
        labels: &[&'static str],
        selected: usize,
        cx: &mut Context<Self>,
        select: fn(&mut Self, usize),
    ) -> Div {
        row()
            .gap(px(3.0))
            .flex_wrap()
            .children(labels.iter().enumerate().map(|(index, &label)| {
                let active = index == selected;
                div()
                    .id((id, index))
                    .px(px(9.0))
                    .py(px(7.0))
                    .rounded(px(4.0))
                    .text_xs()
                    .bg(rgb(if active { 0xeaf0ff } else { WASH }))
                    .text_color(rgb(if active { ACCENT } else { MUTED }))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xe7ebf2)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        select(this, index);
                        this.choice_changed(id, cx);
                        cx.notify();
                    }))
                    .child(label)
            }))
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        row()
            .h(px(66.0))
            .flex_shrink_0()
            .px(px(24.0))
            .gap(px(32.0))
            .bg(rgb(0x20242b))
            .text_color(rgb(0xffffff))
            .child(
                row()
                    .gap(px(10.0))
                    .w(px(244.0))
                    .child(heading("BITMAN").text_lg())
                    .child(div().text_xs().text_color(rgb(0x9199a8)).child("0.1")),
            )
            .child(
                row()
                    .h_full()
                    .gap(px(24.0))
                    .children(Screen::ALL.into_iter().map(|(screen, label)| {
                        let active = self.screen == screen;
                        div()
                            .id(label)
                            .h_full()
                            .flex()
                            .items_center()
                            .border_b_2()
                            .border_color(rgb(if active { 0x98b1ff } else { 0x20242b }))
                            .text_sm()
                            .text_color(rgb(if active { 0xffffff } else { 0xa2a8b3 }))
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(0xffffff)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.screen = screen;
                                this.request_effects(cx);
                                cx.notify();
                            }))
                            .child(label)
                    })),
            )
            .child(div().flex_1())
            .child(
                row()
                    .gap(px(8.0))
                    .child(
                        button("Open image…")
                            .id("open-image")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.open_image(cx))),
                    )
                    .child(
                        primary_button(match self.screen {
                            Screen::Compose => "Save as SVG",
                            Screen::Print => "Save layered SVG",
                            Screen::Simulate => "Save mock-up",
                            _ => "Download SVG",
                        })
                        .id("save-svg")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.save_svg(cx))),
                    ),
            )
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let tabs = match self.screen {
            Screen::Convert => self.choices(
                "preview",
                &["SVG", "Original", "Split", "Overlay", "Batch"],
                self.preview,
                cx,
                |this, index| this.preview = index,
            ),
            Screen::Compose => self.choices(
                "compose-tool",
                &["Select", "Nodes", "Pen", "Text", "Shape", "Fill"],
                self.compose_tool,
                cx,
                |this, index| this.compose_tool = index,
            ),
            Screen::Print => {
                const LABELS: [&str; 12] = [
                    "All plates",
                    "Ink 1",
                    "Ink 2",
                    "Ink 3",
                    "Ink 4",
                    "Ink 5",
                    "Ink 6",
                    "Ink 7",
                    "Ink 8",
                    "Ink 9",
                    "Ink 10",
                    "Ink 11",
                ];
                let labels = &LABELS[..(self.editor.info.plates + 1).clamp(1, 12)];
                self.choices(
                    "plates",
                    labels,
                    self.plate.min(labels.len() - 1),
                    cx,
                    |this, index| this.plate = index,
                )
            }
            Screen::Simulate => row().child(muted("T-shirt preview")),
            Screen::Help => row().child(muted("Workspace guide")),
        };

        row()
            .h(px(49.0))
            .flex_shrink_0()
            .px(px(18.0))
            .border_b_1()
            .border_color(rgb(LINE))
            .bg(rgb(PANEL))
            .when(self.screen == Screen::Convert, |bar| {
                bar.child(label("View"))
            })
            .when(self.screen == Screen::Compose, |bar| {
                bar.child(label("Tools"))
            })
            .when(self.screen == Screen::Print, |bar| {
                bar.child(label("Plates"))
            })
            .child(tabs)
            .when(self.screen == Screen::Compose, |bar| {
                bar.child(muted("│"))
                    .child(button("Undo"))
                    .child(button("Redo"))
            })
            .child(div().flex_1())
            .when(self.screen == Screen::Convert && self.preview != 4, |bar| {
                bar.child(label("Background")).child(self.choices(
                    "background",
                    &["White", "Black", "Clear"],
                    self.background,
                    cx,
                    |this, index| this.background = index,
                ))
            })
            .child(button("Fit"))
            .child(button("100%"))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.screen {
            Screen::Convert => self.convert_sidebar(cx),
            Screen::Compose => self.compose_sidebar(cx),
            Screen::Print => self.print_sidebar(cx),
            Screen::Simulate => self.simulate_sidebar(cx),
            Screen::Help => self.help_sidebar(),
        };
        div()
            .id("inspector")
            .w(px(300.0))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(LINE))
            .bg(rgb(PANEL))
            .child(content)
    }

    fn footer(&self) -> Div {
        if matches!(self.screen, Screen::Convert | Screen::Print) {
            let info = self.editor.info;
            return row()
                .flex_shrink_0()
                .px(px(24.0))
                .py(px(12.0))
                .bg(rgb(PANEL))
                .border_t_1()
                .border_color(rgb(LINE))
                .child(
                    column()
                        .gap(px(3.0))
                        .child(muted(self.editor.status.clone()))
                        .child(muted(format!(
                            "{} paths · {} nodes · {} colours · {} plates",
                            info.paths, info.nodes, info.palette_len, info.plates
                        ))),
                );
        }
        let content = match self.screen {
            Screen::Convert => row()
                .child(stat("Image", "No image loaded"))
                .child(stat("Paths", "—"))
                .child(stat("Nodes", "—"))
                .child(stat("Colours", "—"))
                .child(stat("SVG size", "—")),
            Screen::Print => row()
                .child(stat("Page", "A4 landscape"))
                .child(stat("Plates", "—"))
                .child(stat("Dots", "—"))
                .child(stat("Layered file", "—"))
                .child(stat("Marks", "6 registration marks")),
            Screen::Compose => row()
                .child(stat("Artboard", "297 × 210 mm"))
                .child(stat("Selection", "Nothing selected"))
                .child(stat("Objects", "0")),
            Screen::Simulate => row()
                .child(stat("Designs", "0 on this side"))
                .child(stat("Units", "Centimetres"))
                .child(stat("Placement", "From collar / centre")),
            Screen::Help => row().child(stat("Workspace", "Local image & print tools")),
        };
        div()
            .flex_shrink_0()
            .px(px(24.0))
            .py(px(16.0))
            .bg(rgb(PANEL))
            .border_t_1()
            .border_color(rgb(LINE))
            .child(content.gap(px(24.0)))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.release_retired_images(window);
        div()
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.load_path(path.clone(), cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.move_effect_slider(event, cx)
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.editor.active_slider = None;
                    cx.notify();
                }),
            )
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .font_family(crate::fonts::BODY)
            .text_color(rgb(INK))
            .text_sm()
            .bg(rgb(WASH))
            .child(self.header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(self.toolbar(cx))
                            .child(self.stage(cx))
                            .when(
                                self.screen == Screen::Print
                                    && self.print_render == 0
                                    && !(self.print_method == 2 && self.print_kind == 0),
                                |main| {
                                    main.child(
                                        row()
                                            .justify_end()
                                            .px(px(20.0))
                                            .py(px(9.0))
                                            .child(check("Draft preview", false)),
                                    )
                                },
                            )
                            .child(self.footer()),
                    ),
            )
    }
}
