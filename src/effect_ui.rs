//! Controls for the image effects. Values always live in the engine's Params.
use crate::{
    app::{Screen, Workspace},
    controls::*,
};
use bitman::effects::{self, Method, Params, Pattern, cluster::Algorithm};
use gpui::{
    Context, Div, MouseButton, MouseMoveEvent, ObjectFit, Pixels, Point, Styled, canvas, div, img,
    prelude::*, px, rgb,
};

macro_rules! numbers {
    ($($id:ident: $field:ident, $ty:ty, $min:expr, $max:expr, $step:expr, $unit:literal;)*) => {
        #[derive(Clone,Copy)] #[repr(usize)] pub enum Number {$($id,)*}
        impl Number {
            const ALL:&[Self]=&[$(Self::$id,)*];
            fn spec(self,p:&Params)->(f32,f32,f32,f32,&'static str) {match self {$(Self::$id=>(p.$field as f32,$min,$max,$step,$unit),)*}}
            fn set(self,p:&mut Params,v:f32) {match self {$(Self::$id=>p.$field=v as $ty,)*}}
        }
    }
}
numbers! {
    Upscale:upscale,f32,0.25,4.0,0.25,"×";
    Threshold:threshold,f32,0.0,1.0,1.0/255.0,"";
    Speckle:speckle,usize,0.0,80.0,1.0," px";
    Edge:edge,f32,0.0,3.0,0.05,"";
    Epsilon:epsilon,f32,0.0,4.0,0.05," px";
    Corner:corner,f32,0.0,180.0,1.0,"°";
    Colors:colors,usize,2.0,32.0,1.0,"";
    Black:black,f32,0.0,0.99,0.01,"";
    White:white,f32,0.01,1.0,0.01,"";
    Gamma:gamma,f32,0.2,3.0,0.05,"";
    Contrast:contrast,f32,0.1,3.0,0.05,"";
    Brightness:brightness,f32,-1.0,1.0,0.02,"";
    Smoothing:smoothing,f32,0.0,5.0,0.1," px";
    BlackAmount:black_amount,f32,0.0,1.0,0.01,"";
    Lpi:lpi,f32,5.0,100.0,1.0," LPI";
    Angle:angle,f32,0.0,90.0,0.5,"°";
    Ppi:ppi,f32,72.0,600.0,1.0," px/in";
    Grain:grain,f32,0.05,2.0,0.01," mm";
    PatternSize:pattern_size,f32,3.0,40.0,0.5," grains";
    Inks:inks,usize,1.0,10.0,1.0,"";
    Trap:trap,f32,0.0,1.0,0.01," mm";
    Solid:solid_detect,f32,0.0,1.0,0.01,"";
    Outline:outline_mm,f32,0.0,10.0,0.1," mm";
    Margin:margin,f32,0.0,70.0,0.5," mm";
    ArtWidth:art_width,f32,0.0,400.0,1.0," mm";
}

impl Workspace {
    fn number(
        &self,
        title: &'static str,
        key: Number,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let (v, min, max, step, unit) = key.spec(&self.editor.params);
        let value = if matches!(key, Number::Threshold) && v == 0.0 {
            "Auto".into()
        } else if matches!(key, Number::ArtWidth) && v == 0.0 {
            "Fit page".into()
        } else if step >= 1.0 {
            format!("{v:.0}{unit}")
        } else {
            format!("{v:.2}{unit}")
        };
        let bounds = self.editor.bounds.clone();
        slider(title, value, (v - min) / (max - min))
            .relative()
            .id(("effect-slider", key as usize))
            .cursor_pointer()
            .child(
                canvas(
                    move |rect, _, _| {
                        bounds.borrow_mut()[key as usize] = rect;
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.editor.active_slider = Some(key as usize);
                    this.set_effect_slider(key, event.position, cx);
                }),
            )
    }
    fn set_effect_slider(&mut self, key: Number, point: Point<Pixels>, cx: &mut Context<Self>) {
        let rect = self.editor.bounds.borrow()[key as usize];
        if rect.size.width <= px(0.0) {
            return;
        }
        let (_, min, max, step, _) = key.spec(&self.editor.params);
        let t = ((point.x - rect.origin.x) / rect.size.width).clamp(0.0, 1.0);
        let value = ((min + t * (max - min)) / step).round() * step;
        let previous = self.editor.params;
        key.set(&mut self.editor.params, value.clamp(min, max));
        self.editor.params.black = self
            .editor
            .params
            .black
            .min(self.editor.params.white - 0.004);
        self.editor.params.white = self
            .editor
            .params
            .white
            .max(self.editor.params.black + 0.004);
        self.editor.params.margin = self.editor.params.margin.min(
            (self
                .editor
                .params
                .page_height
                .min(self.editor.params.page_width)
                - 1.0)
                * 0.5,
        );
        if previous != self.editor.params {
            self.request_effects(cx);
        }
    }
    pub fn move_effect_slider(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button != Some(MouseButton::Left) {
            self.editor.active_slider = None;
            return;
        }
        if let Some(index) = self.editor.active_slider {
            self.set_effect_slider(Number::ALL[index], event.position, cx);
        }
    }
    fn toggle(
        &self,
        title: &'static str,
        checked: bool,
        change: fn(&mut Params),
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        check(title, checked)
            .id(title)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                change(&mut this.editor.params);
                this.request_effects(cx);
            }))
    }
    fn effect_button(
        &self,
        title: &'static str,
        change: fn(&mut Params),
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        button(title)
            .id(title)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                change(&mut this.editor.params);
                this.request_effects(cx);
            }))
    }
    fn quantizer(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap(px(6.0))
            .child(label("Palette algorithm"))
            .child(self.choices(
                "quantizer",
                &["Lloyd k-means", "e-means"],
                usize::from(self.editor.params.algorithm == Algorithm::EMeans),
                cx,
                |s, i| {
                    s.editor.params.algorithm = if i == 0 {
                        Algorithm::KMeans
                    } else {
                        Algorithm::EMeans
                    }
                },
            ))
    }
    fn ink_swatches(&self, cx: &mut Context<Self>) -> Div {
        let ink = self.editor.params.ink;
        column()
            .gap(px(6.0))
            .child(muted(format!(
                "Ink #{:02X}{:02X}{:02X}",
                ink[0], ink[1], ink[2]
            )))
            .child(
                row().flex_wrap().children(
                    [
                        0x000000, 0xffffff, 0x315ed8, 0xd44a54, 0xe9bc46, 0x49866c, 0xb49bc8,
                    ]
                    .into_iter()
                    .map(|color| {
                        div()
                            .id(("ink", color as usize))
                            .size(px(24.0))
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(rgb(LINE))
                            .bg(rgb(color))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.editor.params.ink =
                                    [(color >> 16) as u8, (color >> 8) as u8, color as u8];
                                this.request_effects(cx);
                            }))
                    }),
                ),
            )
            .child(
                button(if self.editor.picking {
                    "Cancel colour picker"
                } else {
                    "Pick colour from image"
                })
                .id("pick-ink")
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.editor.picking = !this.editor.picking;
                    this.editor.picking_garment = false;
                    cx.notify();
                })),
            )
    }
    pub fn convert_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.editor.params;
        column()
            .gap_0()
            .child(section(
                "Look",
                column()
                    .child(muted("Clean up scans and trace them as editable SVG."))
                    .child(
                        row()
                            .flex_wrap()
                            .child(self.effect_button(
                                "Clean",
                                |p| {
                                    p.speckle = 12;
                                    p.edge = 1.0;
                                    p.epsilon = 1.2;
                                    p.denoise = true;
                                },
                                cx,
                            ))
                            .child(self.effect_button(
                                "Balanced",
                                |p| {
                                    p.speckle = 6;
                                    p.edge = 0.6;
                                    p.epsilon = 0.75;
                                    p.denoise = true;
                                },
                                cx,
                            ))
                            .child(self.effect_button(
                                "Crunchy",
                                |p| {
                                    p.speckle = 0;
                                    p.edge = 0.0;
                                    p.epsilon = 0.2;
                                    p.denoise = false;
                                },
                                cx,
                            )),
                    ),
            ))
            .child(section(
                "Image size",
                column()
                    .child(self.number("Scale", Number::Upscale, cx))
                    .child(label("Upscale style"))
                    .child(self.choices(
                        "upscale-style",
                        &["Smooth", "Pixel"],
                        self.upscale_style,
                        cx,
                        |s, i| s.upscale_style = i,
                    ))
                    .child(muted("Preview ≤ 1024 px · SVG export ≤ 4096 px")),
            ))
            .child(section(
                "Black & white cut",
                column()
                    .child(self.number("Threshold (zero = auto)", Number::Threshold, cx))
                    .child(self.toggle(
                        "Adaptive threshold",
                        p.adaptive,
                        |p| p.adaptive = !p.adaptive,
                        cx,
                    )),
            ))
            .child(section(
                "Detail",
                column()
                    .child(self.number("Speckle removal", Number::Speckle, cx))
                    .child(self.number("Edge smoothing", Number::Edge, cx))
                    .child(self.number("Simplify", Number::Epsilon, cx))
                    .child(self.number("Corner sharpness", Number::Corner, cx)),
            ))
            .child(section(
                "Colour",
                column()
                    .child(label("Mode"))
                    .child(self.choices(
                        "convert-mode",
                        &["Auto", "Line art", "Colour"],
                        self.convert_mode,
                        cx,
                        |s, i| s.convert_mode = i,
                    ))
                    .when(self.convert_mode != 1, |c| {
                        c.child(self.number("Colours", Number::Colors, cx))
                            .child(self.quantizer(cx))
                    })
                    .child(self.ink_swatches(cx))
                    .child(self.toggle(
                        "Sample ink from scan",
                        p.sample_ink,
                        |p| p.sample_ink = !p.sample_ink,
                        cx,
                    )),
            ))
            .child(section(
                "Cleanup",
                column()
                    .child(self.toggle(
                        "Denoise before tracing",
                        p.denoise,
                        |p| p.denoise = !p.denoise,
                        cx,
                    ))
                    .child(self.toggle(
                        "Flatten uneven paper tone",
                        p.normalize,
                        |p| p.normalize = !p.normalize,
                        cx,
                    )),
            ))
    }
    pub fn print_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.editor.params;
        let flat = self.print_method == 2 && self.print_kind == 0;
        let screen = self.print_render == 0;
        column()
            .gap_0()
            .child(section(
                "Method",
                column()
                    .child(label("Colour separation"))
                    .child(self.choices(
                        "print-method",
                        &["One ink", "CMYK", "Spot colour"],
                        self.print_method,
                        cx,
                        |s, i| s.print_method = i,
                    ))
                    .when(self.print_method == 2, |c| {
                        c.child(label("Image type")).child(self.choices(
                            "print-kind",
                            &["Flat graphic", "Photo"],
                            self.print_kind,
                            cx,
                            |s, i| s.print_kind = i,
                        ))
                    }),
            ))
            .child(section(
                "Levels",
                column()
                    .child(self.histogram_view())
                    .child(self.number("Black point", Number::Black, cx))
                    .child(self.number("Midtones", Number::Gamma, cx))
                    .child(self.number("White point", Number::White, cx))
                    .child(
                        button("Auto levels")
                            .id("auto-levels")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.auto_levels(cx))),
                    )
                    .child(self.effect_button(
                        "Reset levels",
                        |p| {
                            p.black = 0.0;
                            p.white = 1.0;
                            p.gamma = 1.0;
                        },
                        cx,
                    )),
            ))
            .when(!flat, |side| {
                side.child(section(
                    "Render",
                    column()
                        .child(label("Rendering method"))
                        .child(self.choices(
                            "print-render",
                            &["Screen halftone", "Dither"],
                            self.print_render,
                            cx,
                            |s, i| s.print_render = i,
                        ))
                        .when(screen, |c| {
                            c.child(self.number("Frequency", Number::Lpi, cx))
                                .when(self.print_method == 0, |c| {
                                    c.child(self.number("Angle", Number::Angle, cx))
                                })
                                .child(label("Dot shape"))
                                .child(self.choices(
                                    "print-shape",
                                    &[
                                        "Round", "Diamond", "Ellipse", "Line", "Square", "Cross",
                                        "Custom",
                                    ],
                                    self.print_shape,
                                    cx,
                                    |s, i| s.print_shape = i,
                                ))
                                .child(self.number("Output resolution", Number::Ppi, cx))
                        })
                        .when(!screen, |c| {
                            c.child(label("Dither algorithm"))
                                .child(self.choices(
                                    "dither",
                                    &[
                                        "Floyd–Steinberg",
                                        "Atkinson",
                                        "Stucki",
                                        "Bayer",
                                        "Blue noise",
                                        "Serpentine",
                                        "Random",
                                    ],
                                    self.dither,
                                    cx,
                                    |s, i| s.dither = i,
                                ))
                                .child(self.number("Dither size", Number::Grain, cx))
                                .when(self.dither == 5, |c| {
                                    c.child(label("Pattern"))
                                        .child(self.choices(
                                            "pattern",
                                            &["Mazes", "Coral", "Fingerprints", "Loops", "Spots"],
                                            p.pattern as usize,
                                            cx,
                                            |s, i| {
                                                s.editor.params.pattern = [
                                                    Pattern::Mazes,
                                                    Pattern::Coral,
                                                    Pattern::Fingerprints,
                                                    Pattern::Loops,
                                                    Pattern::Spots,
                                                ][i]
                                            },
                                        ))
                                        .child(self.number("Pattern size", Number::PatternSize, cx))
                                })
                        }),
                ))
                .when(screen && self.print_shape == 6, |c| {
                    c.child(section(
                        "Pattern creator",
                        column()
                            .child(muted("Click squares to draw or erase the stamp."))
                            .child(self.stamp_grid(cx))
                            .child(
                                row()
                                    .child(self.effect_button(
                                        "Clear stamp",
                                        |p| p.stamp = [0; 16],
                                        cx,
                                    ))
                                    .child(self.effect_button(
                                        "Reset stamp",
                                        |p| p.stamp = Params::default().stamp,
                                        cx,
                                    )),
                            ),
                    ))
                })
                .child(section(
                    "Tone",
                    column()
                        .child(self.number("Contrast", Number::Contrast, cx))
                        .child(self.number("Brightness", Number::Brightness, cx))
                        .child(self.number("Soften before screening", Number::Smoothing, cx))
                        .when(self.print_method == 1, |c| {
                            c.child(self.number("Black amount", Number::BlackAmount, cx))
                        })
                        .child(self.toggle("Invert", p.invert, |p| p.invert = !p.invert, cx)),
                ))
            })
            .child(section(
                "Ink colours",
                column()
                    .when(self.print_method == 0, |c| c.child(self.ink_swatches(cx)))
                    .when(self.print_method == 1, |c| {
                        c.child(muted("Cyan 15° · Magenta 75° · Yellow 0° · Black 45°"))
                    })
                    .when(self.print_method == 2, |c| {
                        c.child(self.number("Number of inks", Number::Inks, cx))
                            .child(self.quantizer(cx))
                            .child(
                                row()
                                    .child(self.effect_button(
                                        "Automatic inks",
                                        |p| p.picked = 0,
                                        cx,
                                    ))
                                    .child(
                                        button("Pick inks…")
                                            .id("pick-spots")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.editor.picking = !this.editor.picking;
                                                this.editor.picking_garment = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(muted(format!(
                                "{} picked inks · click the image to add",
                                p.picked
                            )))
                            .child(self.toggle(
                                "Do not print paper",
                                p.paper,
                                |p| p.paper = !p.paper,
                                cx,
                            ))
                            .when(flat, |c| c.child(self.number("Trap", Number::Trap, cx)))
                            .when(!flat, |c| {
                                c.child(self.toggle(
                                    "Keep solid areas solid",
                                    p.keep_solid,
                                    |p| p.keep_solid = !p.keep_solid,
                                    cx,
                                ))
                                .child(self.number(
                                    "Solid detection",
                                    Number::Solid,
                                    cx,
                                ))
                            })
                    }),
            ))
            .when(self.print_method == 0 || flat, |c| {
                c.child(section(
                    "Dark garment",
                    column()
                        .child(self.toggle(
                            "Print on a dark garment",
                            p.dark,
                            |p| p.dark = !p.dark,
                            cx,
                        ))
                        .child(muted(format!(
                            "Garment #{:02X}{:02X}{:02X}",
                            p.dark_bg[0], p.dark_bg[1], p.dark_bg[2]
                        )))
                        .child(
                            button("Pick garment colour…")
                                .id("pick-garment")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.editor.picking = !this.editor.picking;
                                    this.editor.picking_garment = this.editor.picking;
                                    cx.notify();
                                })),
                        )
                        .child(self.toggle(
                            "Invert design for garment",
                            p.dark_invert,
                            |p| p.dark_invert = !p.dark_invert,
                            cx,
                        ))
                        .child(self.number("Outline", Number::Outline, cx))
                        .child(self.toggle(
                            "White underbase",
                            p.underbase,
                            |p| p.underbase = !p.underbase,
                            cx,
                        )),
                ))
            })
            .child(section(
                "Design size",
                column()
                    .child(self.number("Artwork width (zero = fit)", Number::ArtWidth, cx))
                    .child(muted("Aspect ratio is preserved. Artwork is centred.")),
            ))
            .child(section(
                "Page",
                column()
                    .child(label("Page size"))
                    .child(
                        row()
                            .child(self.effect_button(
                                "A5",
                                |p| {
                                    p.page_width = 210.0;
                                    p.page_height = 148.0;
                                },
                                cx,
                            ))
                            .child(self.effect_button(
                                "A4",
                                |p| {
                                    p.page_width = 297.0;
                                    p.page_height = 210.0;
                                },
                                cx,
                            ))
                            .child(self.effect_button(
                                "A3",
                                |p| {
                                    p.page_width = 420.0;
                                    p.page_height = 297.0;
                                },
                                cx,
                            )),
                    )
                    .child(self.effect_button(
                        "Rotate page",
                        |p| std::mem::swap(&mut p.page_width, &mut p.page_height),
                        cx,
                    ))
                    .child(muted(format!(
                        "{:.0} × {:.0} mm",
                        p.page_width, p.page_height
                    )))
                    .child(self.number("Artwork margin", Number::Margin, cx)),
            ))
            .child(section(
                "Registration",
                column()
                    .child(self.toggle("Registration marks", p.marks, |p| p.marks = !p.marks, cx))
                    .child(self.toggle(
                        "Six marks (off = four corners)",
                        p.six_marks,
                        |p| p.six_marks = !p.six_marks,
                        cx,
                    ))
                    .child(self.toggle(
                        "Ink name on each plate",
                        p.ink_names,
                        |p| p.ink_names = !p.ink_names,
                        cx,
                    ))
                    .child(muted("Export: one layered SVG, black film per layer.")),
            ))
    }
    fn histogram_view(&self) -> Div {
        let histogram = self.editor.histogram;
        div().w_full().h(px(64.0)).bg(rgb(WASH)).child(
            canvas(
                |_, _, _| (),
                move |rect, (), window, _| {
                    let max = histogram.iter().copied().max().unwrap_or(1).max(1) as f32;
                    for (i, &v) in histogram.iter().enumerate() {
                        let width = rect.size.width / 256.0;
                        let height = rect.size.height * (v as f32 / max).sqrt();
                        let bounds = gpui::Bounds::new(
                            rect.origin + gpui::point(width * i as f32, rect.size.height - height),
                            gpui::size(width, height),
                        );
                        window.paint_quad(gpui::fill(bounds, rgb(0x9baacb)));
                    }
                },
            )
            .size_full(),
        )
    }
    fn stamp_grid(&self, cx: &mut Context<Self>) -> Div {
        let mut grid = column().gap(px(1.0));
        for y in 0..16 {
            let mut line = row().gap(px(1.0));
            for x in 0..16 {
                line = line.child(
                    div()
                        .id(("stamp", y * 16 + x))
                        .size(px(14.0))
                        .bg(rgb(if self.editor.params.stamp[y] & (1 << x) != 0 {
                            INK
                        } else {
                            WASH
                        }))
                        .border_1()
                        .border_color(rgb(LINE))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.editor.params.stamp[y] ^= 1 << x;
                            this.request_effects(cx);
                        })),
                );
            }
            grid = grid.child(line);
        }
        grid
    }
    pub fn image_stage(&self, cx: &mut Context<Self>) -> Div {
        let source = self.editor.source.as_ref().unwrap().clone();
        let preview = self.editor.preview.as_ref().unwrap_or(&source).clone();
        let original = self.editor.picking || (self.screen == Screen::Convert && self.preview == 1);
        let bounds = self.editor.bounds.clone();
        let bg = if self.background == 1 {
            0x20242b
        } else if self.background == 2 {
            0xd9dde4
        } else {
            0xffffff
        };
        let mut content = div()
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .h(px(0.0))
            .w_full()
            .overflow_hidden()
            .bg(rgb(bg));
        if self.screen == Screen::Convert && self.preview == 2 && !self.editor.picking {
            content = content
                .flex()
                .child(
                    div()
                        .relative()
                        .w(gpui::relative(0.5))
                        .h_full()
                        .min_w_0()
                        .child(
                            img(source.clone())
                                .absolute()
                                .inset_0()
                                .size_full()
                                .min_w_0()
                                .min_h_0()
                                .object_fit(ObjectFit::Contain),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .w(gpui::relative(0.5))
                        .h_full()
                        .min_w_0()
                        .child(
                            img(preview)
                                .absolute()
                                .inset_0()
                                .size_full()
                                .min_w_0()
                                .min_h_0()
                                .object_fit(ObjectFit::Contain),
                        ),
                );
        } else {
            content = content.child(
                img(if original { source.clone() } else { preview })
                    .absolute()
                    .inset_0()
                    .min_w_0()
                    .min_h_0()
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            );
            if self.screen == Screen::Convert && self.preview == 3 && !self.editor.picking {
                content = content.child(
                    div().absolute().inset_0().opacity(0.5).child(
                        img(source)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .min_w_0()
                            .min_h_0()
                            .object_fit(ObjectFit::Contain),
                    ),
                );
            }
        }
        column()
            .gap(px(8.0))
            .flex_1()
            .min_h_0()
            .p(px(20.0))
            .child(muted(if self.editor.picking {
                "Colour picker: click the original image".into()
            } else {
                self.editor
                    .path
                    .as_ref()
                    .map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_default()
            }))
            .child(
                content
                    .child(
                        canvas(
                            move |rect, _, _| {
                                bounds.borrow_mut()[63] = rect;
                            },
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.pick_image_colour(event.position, cx)
                        }),
                    ),
            )
    }
    fn pick_image_colour(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.editor.picking {
            return;
        }
        let Some(source) = self.editor.source.as_ref() else {
            return;
        };
        let rect = self.editor.bounds.borrow()[63];
        let size = source.size(0);
        let (w, h) = (size.width.0 as f32, size.height.0 as f32);
        let scale = (f32::from(rect.size.width) / w).min(f32::from(rect.size.height) / h);
        let x = (f32::from(point.x - rect.origin.x)
            - (f32::from(rect.size.width) - w * scale) * 0.5)
            / scale;
        let y = (f32::from(point.y - rect.origin.y)
            - (f32::from(rect.size.height) - h * scale) * 0.5)
            / scale;
        if !(0.0..w).contains(&x) || !(0.0..h).contains(&y) {
            return;
        }
        let Some(bytes) = source.as_bytes(0) else {
            return;
        };
        let Ok(view) = effects::ImageView::new(bytes, w as usize, h as usize, w as usize * 4)
        else {
            return;
        };
        let color = view.rgb(y as usize * w as usize + x as usize);
        if self.editor.picking_garment {
            self.editor.params.dark_bg = color;
            self.editor.picking = false;
            self.editor.picking_garment = false;
        } else if self.screen == Screen::Print && self.editor.params.method == Method::Spot {
            let p = &mut self.editor.params;
            if p.picked < 10 {
                p.ink_colors[p.picked] = color;
                p.picked += 1;
            }
        } else {
            self.editor.params.ink = color;
            self.editor.picking = false;
        }
        self.request_effects(cx);
    }
}
