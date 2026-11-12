use gpui::{
    Context, Div, FocusHandle, IntoElement, PromptLevel, Render, Styled, Window, div, prelude::*,
    px, rgb,
};

use crate::controls::*;
use crate::menus::*;

const SIDEBAR_WIDTH: f32 = 300.0;

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

pub struct Workspace {
    pub focus_handle: FocusHandle,
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
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            editor: crate::editor::Editor::default(),
            screen: Screen::default(),
            preview: 0,
            background: 0,
            convert_mode: 0,
            upscale_style: 0,
            print_method: 0,
            print_kind: 0,
            print_render: 0,
            plate: 0,
            print_shape: 0,
            dither: 0,
            compose_tool: 0,
            library: 0,
            shirt_side: 0,
            shirt_size: 3,
            sleeve: 0,
        }
    }

    fn show_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.screen = screen;
        self.request_effects(cx);
        cx.notify();
    }

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
                    .bg(rgb(if active { ACCENT } else { BUTTON }))
                    .text_color(rgb(if active { WHITE } else { INK }))
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(if active { ACCENT } else { WHITE })))
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
            .pr(px(24.0))
            .gap_0()
            .border_b_1()
            .border_color(rgb(LINE))
            .bg(rgb(PANEL))
            .text_color(rgb(INK))
            .child(
                row()
                    .h_full()
                    .px(px(16.0))
                    .bg(rgb(ACCENT))
                    .text_color(rgb(WHITE))
                    .gap(px(10.0))
                    .w(px(SIDEBAR_WIDTH))
                    .flex_shrink_0()
                    .child(heading("BITMAN").text_lg())
                    .child(div().text_xs().text_color(rgb(BUTTON)).child("0.1")),
            )
            .child(row().h_full().gap_0().children(Screen::ALL.into_iter().map(
                |(screen, label)| {
                    let active = self.screen == screen;
                    div()
                        .id(label)
                        .h_full()
                        .px(px(12.0))
                        .flex()
                        .items_center()
                        .border_b_2()
                        .border_color(rgb(if active { INK } else { PANEL }))
                        .bg(rgb(if active { ACCENT } else { PANEL }))
                        .text_sm()
                        .text_color(rgb(if active { WHITE } else { INK }))
                        .cursor_pointer()
                        .hover(move |style| style.bg(rgb(if active { ACCENT } else { BUTTON })))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.show_screen(screen, cx);
                        }))
                        .child(label)
                },
            )))
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
            .w(px(SIDEBAR_WIDTH))
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
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &OpenImage, _, cx| this.open_image(cx)))
            .when(self.can_export_svg(), |view| {
                view.on_action(cx.listener(|this, _: &ExportSvg, _, cx| this.save_svg(cx)))
            })
            .on_action(cx.listener(|this, _: &ShowConvert, _, cx| this.show_screen(Screen::Convert, cx)))
            .on_action(cx.listener(|this, _: &ShowPrint, _, cx| this.show_screen(Screen::Print, cx)))
            .on_action(cx.listener(|this, _: &ShowHelp, _, cx| this.show_screen(Screen::Help, cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
            .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
            .on_action(cx.listener(|_, _: &ToggleFullScreen, window, _| window.toggle_fullscreen()))
            .on_action(cx.listener(|_, _: &About, window, cx| {
                let detail = format!(
                    "Version {}\n\nLocal image conversion and print preparation.\n\nBased on Boot Tool by sshhiiff and contributors. See Help → Bitman Help for full credits and source links.\n\nFonts: Departure Mono and Sometype Mono, licensed under the SIL Open Font License 1.1.",
                    env!("CARGO_PKG_VERSION")
                );
                drop(window.prompt(PromptLevel::Info, "About Bitman", Some(&detail), &["OK"], cx));
            }))
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
