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
    pub update_state: crate::updater::UpdateState,
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
    pub simulation: crate::simulate_ui::SimulationUi,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut workspace = Self {
            update_state: crate::updater::UpdateState::default(),
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
            simulation: crate::simulate_ui::SimulationUi::default(),
        };
        workspace.start_update_checks(cx);
        workspace
    }

    fn show_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.end_simulation_drag();
        self.screen = screen;
        if screen == Screen::Simulate {
            self.render_simulation(cx);
        }
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
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(BUTTON))
                            .child(env!("CARGO_PKG_VERSION")),
                    ),
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
                        .when(
                            self.screen != Screen::Simulate || self.can_export_mockup(),
                            |b| {
                                b.on_click(cx.listener(|this, _, _, cx| {
                                    if this.screen == Screen::Simulate {
                                        this.save_mockup(cx);
                                    } else {
                                        this.save_svg(cx);
                                    }
                                }))
                            },
                        )
                        .when(
                            self.screen == Screen::Simulate && !self.can_export_mockup(),
                            |b| b.opacity(0.45),
                        ),
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
            .child(
                button("Fit")
                    .id("fit-view")
                    .when(self.screen == Screen::Simulate, |b| {
                        b.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                            this.simulation.actual_size = false;
                            cx.notify();
                        }))
                    }),
            )
            .child(
                button("100%")
                    .id("actual-view")
                    .when(self.screen == Screen::Simulate, |b| {
                        b.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                            this.simulation.actual_size = true;
                            cx.notify();
                        }))
                    }),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.screen {
            Screen::Convert => self.convert_sidebar(cx),
            Screen::Compose => self.compose_sidebar(cx),
            Screen::Print => self.print_sidebar(cx),
            Screen::Simulate => self.simulate_sidebar(cx),
            Screen::Help => self.help_sidebar(cx),
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
            .when(
                matches!(self.screen, Screen::Convert | Screen::Print),
                |side| {
                    side.child(section(
                        "Mock-up",
                        button("Add to Simulate")
                            .id("send-to-simulate")
                            .when(self.can_export_svg(), |b| {
                                b.cursor_pointer().on_click(
                                    cx.listener(|this, _, _, cx| this.send_to_simulate(cx)),
                                )
                            })
                            .when(!self.can_export_svg(), |b| b.opacity(0.45)),
                    ))
                },
            )
            .child(content)
    }

    fn footer(&self) -> Div {
        if self.screen == Screen::Simulate {
            return column()
                .flex_shrink_0()
                .px(px(20.0))
                .py(px(10.0))
                .bg(rgb(PANEL))
                .border_t_1()
                .border_color(rgb(LINE))
                .child(muted(format!(
                    "{} design(s) on {} · centimetres · {}",
                    self.simulation.scene.current().len(),
                    if self.simulation.scene.back {
                        "back"
                    } else {
                        "front"
                    },
                    if self.simulation.actual_size {
                        "100% (preview pixels)"
                    } else {
                        "Fit"
                    }
                )))
                .child(muted(self.simulation.status.clone()));
        }
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
            .on_action(cx.listener(|this, _: &CheckForUpdates, _, cx| {
                this.screen = Screen::Help;
                this.check_for_updates(cx);
                cx.notify();
            }))
            .when(self.can_export_svg(), |view| {
                view.on_action(cx.listener(|this, _: &ExportSvg, _, cx| this.save_svg(cx)))
            })
            .on_action(cx.listener(|this, _: &ShowConvert, _, cx| this.show_screen(Screen::Convert, cx)))
            .on_action(cx.listener(|this, _: &ShowPrint, _, cx| this.show_screen(Screen::Print, cx)))
            .on_action(cx.listener(|this, _: &ShowSimulate, _, cx| this.show_screen(Screen::Simulate, cx)))
            .when(self.screen == Screen::Simulate && self.can_export_mockup(), |view| view.on_action(cx.listener(|this, _: &ExportMockup, _, cx| this.save_mockup(cx))))
            .when(self.screen == Screen::Simulate && self.simulation.scene.selection().is_some(), |view| view.on_action(cx.listener(|this, _: &RemoveArtwork, _, cx| this.remove_simulation_design(cx))))
            .on_action(cx.listener(|this, _: &ShowHelp, _, cx| this.show_screen(Screen::Help, cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| {
                if !crate::updater::install_in_progress() {
                    window.remove_window();
                }
            }))
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
                if this.screen == Screen::Simulate {
                    for path in paths.paths() { this.load_simulation_path(path.clone(), cx); }
                } else if let Some(path) = paths.paths().first() {
                    this.load_path(path.clone(), cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.move_effect_slider(event, cx);
                this.move_simulation_pointer(event, cx);
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.editor.active_slider = None;
                    this.end_simulation_drag();
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
            .when(matches!(self.update_state, crate::updater::UpdateState::Available(_) | crate::updater::UpdateState::Installing | crate::updater::UpdateState::Installed | crate::updater::UpdateState::InstallFailed(_)), |view| view.child(self.update_banner(cx)))
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
                            .child(self.stage(window.viewport_size().width - px(SIDEBAR_WIDTH), cx))
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
