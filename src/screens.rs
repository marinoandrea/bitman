use gpui::{
    Context, Div, FontWeight, PathBuilder, Styled, canvas, div, point, prelude::*, px, rgb,
};

use crate::{
    app::{Screen, Workspace},
    controls::*,
};

impl Workspace {
    pub fn convert_sidebar(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap_0()
            .when(self.preview == 4, |side| {
                side.child(section(
                    "Batch convert",
                    column()
                        .child(
                            row()
                                .child(button("Add images…"))
                                .child(button("Add folder…")),
                        )
                        .child(labeled_input(
                            "Output folder",
                            "Choose a folder on this computer",
                        ))
                        .child(label("Settings"))
                        .child(
                            row()
                                .child(button("Same for all"))
                                .child(button("Per image")),
                        )
                        .child(check("Keep going if an image fails", true))
                        .child(primary_button("Convert and save to folder")),
                ))
            })
            .child(section(
                "Look",
                column()
                    .child(muted("Choose how clean or textured your vector looks."))
                    .child(slider("Clean ← → Crunchy", "50", 0.5))
                    .child(
                        row()
                            .child(button("Clean"))
                            .child(button("Balanced"))
                            .child(button("Crunchy")),
                    ),
            ))
            .child(section(
                "Image size",
                column()
                    .child(slider("Upscale", "1×", 0.14))
                    .child(label("Upscale style"))
                    .child(self.choices(
                        "upscale-style",
                        &["Smooth", "Pixel"],
                        self.upscale_style,
                        cx,
                        |s, i| s.upscale_style = i,
                    ))
                    .child(muted("Smaller for texture. Larger for finer edges.")),
            ))
            .child(section(
                "Black & white cut",
                column()
                    .child(slider("Threshold", "Auto", 0.0))
                    .child(label("Threshold method"))
                    .child(row().child(button("Automatic")).child(button("Adaptive"))),
            ))
            .child(section(
                "Detail",
                column()
                    .child(slider("Speckle removal", "6 px", 0.075))
                    .child(slider("Edge smoothing", "0.60", 0.3))
                    .child(slider("Simplify", "0.75", 0.18))
                    .child(slider("Corner sharpness", "125°", 0.6)),
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
                    .when(self.convert_mode != 1, |body| {
                        body.child(slider("Colours", "8", 0.43))
                    })
                    .child(field("Ink colour", "■  #000000"))
                    .child(check("Sample from scan", false)),
            ))
            .child(section(
                "Cleanup",
                column()
                    .child(check("Denoise before tracing", true))
                    .child(check("Flatten uneven paper tone", true)),
            ))
            .child(section(
                "Command line",
                column()
                    .child(input("boottool run ./images ./out"))
                    .child(button("Copy command")),
            ))
    }

    pub fn compose_sidebar(&self, _cx: &mut Context<Self>) -> Div {
        column()
            .gap_0()
            .child(section(
                "Artboard",
                column()
                    .child(label("Page preset"))
                    .child(
                        row()
                            .child(button("A5"))
                            .child(button("A4"))
                            .child(button("A3")),
                    )
                    .child(label("Orientation"))
                    .child(row().child(button("Landscape")).child(button("Portrait")))
                    .child(field("Width", "297 mm"))
                    .child(field("Height", "210 mm")),
            ))
            .child(section(
                "Layers",
                column()
                    .child(muted("Your page is empty"))
                    .child(row().child(button("Group")).child(button("Ungroup"))),
            ))
            .child(section(
                "Transform",
                column()
                    .child(muted("Select an object to edit its position"))
                    .child(field("X / Y", "— / — mm"))
                    .child(field("Width / height", "— / — mm"))
                    .child(field("Rotation", "0°"))
                    .child(row().child(button("Flip ↔")).child(button("Flip ↕")))
                    .child(row().child(button("Centre ↔")).child(button("Centre ↕")))
                    .child(row().child(button("To front")).child(button("To back"))),
            ))
            .child(section(
                "Colour",
                column()
                    .child(field("Fill", "■  #000000"))
                    .child(field("Stroke", "None"))
                    .child(field("Weight", "0.5 mm"))
                    .child(swatches())
                    .child(check("Apply to whole page", false)),
            ))
            .when(self.compose_tool == 3, |side| {
                side.child(section(
                    "Text",
                    column()
                        .child(labeled_input("Text content", "Type something…"))
                        .child(field("Font", "System font  ▾"))
                        .child(field("Size", "48 pt"))
                        .child(field("Letter spacing", "0"))
                        .child(label("Alignment"))
                        .child(
                            row()
                                .child(button("Left"))
                                .child(button("Centre"))
                                .child(button("Right")),
                        ),
                ))
            })
            .when(self.compose_tool == 1, |side| {
                side.child(section(
                    "Nodes",
                    column()
                        .child(row().child(button("Corner")).child(button("Smooth")))
                        .child(
                            row()
                                .child(button("Break path"))
                                .child(button("Delete node")),
                        )
                        .child(field("Simplify", "0.05 mm"))
                        .child(button("Simplify selection")),
                ))
            })
            .when(self.compose_tool == 4, |side| {
                side.child(section(
                    "Shape",
                    column()
                        .child(row().child(button("Rectangle")).child(button("Ellipse")))
                        .child(row().child(button("Polygon")).child(button("Star")))
                        .child(field("Sides", "5")),
                ))
            })
            .when(self.compose_tool == 5, |side| {
                side.child(section(
                    "Fill tool",
                    column()
                        .child(field("Grow", "0.2 mm"))
                        .child(field("Close gaps", "0.0 mm"))
                        .child(muted("Click an enclosed area to fill it.")),
                ))
            })
            .child(section(
                "Combine shapes",
                column()
                    .child(
                        row()
                            .flex_wrap()
                            .child(button("Unite"))
                            .child(button("Subtract"))
                            .child(button("Intersect")),
                    )
                    .child(
                        row()
                            .child(button("Exclude"))
                            .child(button("Split"))
                            .child(button("Combine")),
                    )
                    .child(row().child(button("Separate")).child(button("Join ends"))),
            ))
            .child(section(
                "Output",
                column()
                    .child(primary_button("Send to Print"))
                    .child(button("Save as SVG")),
            ))
    }

    pub fn print_sidebar(&self, cx: &mut Context<Self>) -> Div {
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
                    .when(self.print_method == 2, |body| {
                        body.child(label("Image type")).child(self.choices(
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
                    .child(
                        div()
                            .h(px(74.0))
                            .rounded(px(4.0))
                            .bg(rgb(WASH))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(muted("No image histogram")),
                    )
                    .child(
                        row()
                            .justify_between()
                            .child(labeled_input("Black point", "0"))
                            .child(labeled_input("Midtones", "1.00"))
                            .child(labeled_input("White point", "255")),
                    )
                    .child(row().child(button("Auto")).child(button("Reset"))),
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
                        .when(screen, |body| {
                            body.child(slider("Frequency", "30 LPI", 0.26))
                                .when(self.print_method == 0, |body| {
                                    body.child(slider("Angle", "22.5°", 0.25))
                                })
                                .child(muted("Shape"))
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
                                .child(slider("Output resolution", "300 px/in", 0.33))
                        })
                        .when(!screen, |body| {
                            body.child(self.choices(
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
                            .child(slider("Dither size", "0.30 mm", 0.1))
                            .when(self.dither == 5, |body| {
                                body.child(field("Pattern", "Mazes  ▾")).child(slider(
                                    "Pattern size",
                                    "9 grains",
                                    0.16,
                                ))
                            })
                        }),
                ))
                .when(screen && self.print_shape == 6, |side| {
                    side.child(section(
                        "Pattern creator",
                        column()
                            .child(pattern_grid())
                            .child(
                                row()
                                    .child(button("Draw"))
                                    .child(button("Erase"))
                                    .child(button("Clear")),
                            )
                            .child(button("Import SVG / PNG…")),
                    ))
                })
                .child(section(
                    "Tone",
                    column()
                        .child(slider("Contrast", "1.00", 0.26))
                        .child(slider("Brightness", "0.00", 0.5))
                        .child(slider("Soften before screening", "0.0", 0.0))
                        .when(self.print_method == 1, |body| {
                            body.child(slider("Black amount", "60%", 0.6))
                        })
                        .child(check("Invert", false)),
                ))
            })
            .child(section(
                if self.print_method == 1 {
                    "CMYK plates"
                } else {
                    "Ink colours"
                },
                column()
                    .when(self.print_method == 0, |body| {
                        body.child(field("Ink", "■  #000000"))
                    })
                    .when(self.print_method == 1, |body| {
                        body.child(field("Cyan", "■  15°"))
                            .child(field("Magenta", "■  75°"))
                            .child(field("Yellow", "■  0°"))
                            .child(field("Black", "■  45°"))
                    })
                    .when(self.print_method == 2, |body| {
                        body.child(field("Number of inks", "4"))
                            .child(
                                row()
                                    .child(button("Automatic"))
                                    .child(button("Pick from image")),
                            )
                            .child(swatches())
                            .child(check("Do not print paper", true))
                            .when(flat, |body| body.child(slider("Trap", "0.20 mm", 0.2)))
                            .when(!flat, |body| {
                                body.child(check("Keep solid areas solid", false))
                                    .child(slider("Solid detection", "50%", 0.5))
                            })
                    }),
            ))
            .when(self.print_method == 0 || flat, |side| {
                side.child(section(
                    "Dark garment",
                    column()
                        .child(check("Print on a dark garment", false))
                        .child(field("Garment colour", "■  #1b1b1d"))
                        .child(check("Invert design for garment", false))
                        .child(check("White underbase", false)),
                ))
            })
            .child(section(
                "Design size",
                column()
                    .child(row().child(button("Fit to page")).child(button("Set size")))
                    .child(muted("Keep proportions and centre on the page.")),
            ))
            .child(section(
                "Page",
                column()
                    .child(label("Page size"))
                    .child(
                        row()
                            .child(button("A5"))
                            .child(button("A4"))
                            .child(button("A3")),
                    )
                    .child(label("Orientation"))
                    .child(row().child(button("Portrait")).child(button("Landscape")))
                    .child(slider("Artwork margin", "28 mm", 0.26)),
            ))
            .child(section(
                "Registration",
                column()
                    .child(check("Registration marks", true))
                    .child(label("Mark layout"))
                    .child(
                        row()
                            .child(button("Six marks"))
                            .child(button("Four corners")),
                    )
                    .child(field("Mark size", "10 mm"))
                    .child(field("Weight", "2 pt"))
                    .child(check("Ink name on each plate", true)),
            ))
            .child(section(
                "Output",
                column()
                    .child(primary_button("Download layered SVG"))
                    .child(button("Download plates as ZIP"))
                    .child(button("Save to Print library")),
            ))
    }

    pub fn simulate_sidebar(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap_0()
            .child(section(
                "Shirt",
                column()
                    .child(label("Shirt size"))
                    .child(self.choices(
                        "shirt-size",
                        &["XS", "S", "M", "L", "XL", "XXL"],
                        self.shirt_size,
                        cx,
                        |s, i| s.shirt_size = i,
                    ))
                    .child(label("Sleeve length"))
                    .child(self.choices(
                        "sleeve",
                        &["Short sleeve", "Long sleeve"],
                        self.sleeve,
                        cx,
                        |s, i| s.sleeve = i,
                    ))
                    .child(label("View"))
                    .child(self.choices(
                        "shirt-side",
                        &["Front", "Back"],
                        self.shirt_side,
                        cx,
                        |s, i| s.shirt_side = i,
                    )),
            ))
            .child(section(
                "Shirt colour",
                column().child(swatches()).child(button("Use Print colour")),
            ))
            .child(section(
                "Designs",
                column()
                    .child(muted("No designs on this side"))
                    .child(muted("Choose a graphic from the library below.")),
            ))
            .child(section(
                "Placement",
                column()
                    .child(field("Width", "— cm"))
                    .child(field("Height", "— cm"))
                    .child(field("From collar", "— cm"))
                    .child(field("From centre", "— cm"))
                    .child(field("Rotation", "0°"))
                    .child(check("Keep proportions", true))
                    .child(
                        row()
                            .child(button("Centre"))
                            .child(button("Chest"))
                            .child(button("Full front")),
                    )
                    .child(row().child(button("Duplicate")).child(button("Remove"))),
            ))
            .child(section(
                "View & export",
                column()
                    .child(check("Show shirt measurements", true))
                    .child(check("Crop designs to the shirt", true))
                    .child(check("Include measurements in JPG", false))
                    .child(primary_button("Save mock-up as JPG")),
            ))
    }

    pub fn help_sidebar(&self) -> Div {
        column()
            .gap_0()
            .child(section(
                "Getting started",
                column()
                    .child(muted("01   Convert a raster image"))
                    .child(muted("02   Compose a page"))
                    .child(muted("03   Prepare print plates"))
                    .child(muted("04   Preview on a garment")),
            ))
            .child(section(
                "Supported images",
                column().child(muted("PNG · JPG · GIF · WebP · BMP · TIFF")),
            ))
    }

    pub fn stage(&self, cx: &mut Context<Self>) -> Div {
        let stage = div().flex().flex_col().flex_1().min_h_0().overflow_hidden();
        match self.screen {
            Screen::Convert if self.preview == 4 => stage.child(self.batch_stage()),
            Screen::Convert => stage.items_center().justify_center().child(
                column().items_center().gap(px(22.0)).p(px(48.0))
                    .child(div().size(px(54.0)).rounded(px(12.0)).bg(rgb(0xe7ebf3)).flex().items_center().justify_center().text_2xl().text_color(rgb(0x8999b7)).child("↥"))
                    .child(empty_message("Drop an image to get started", "or open an image from your computer"))
                    .child(muted("PNG, JPG, GIF, WebP, BMP, TIFF"))
                    .child(button("Choose image…"))),
            Screen::Compose => stage
                .child(div().flex().flex_1().min_h_0().p(px(32.0)).items_center().justify_center()
                    .child(column().gap(px(8.0)).size_full().max_w(px(740.0))
                        .child(row().justify_between().child(muted("0"))
                            .child(muted("50"))
                            .child(muted("100"))
                            .child(muted("150"))
                            .child(muted("200"))
                            .child(muted("250 mm")))
                        .child(artboard())))
                .child(self.library_shelf(cx)),
            Screen::Print => stage.items_center().justify_center().child(
                column().items_center().gap(px(20.0))
                    .child(div().w(px(480.0)).h(px(340.0)).bg(rgb(PANEL)).border_1().border_color(rgb(LINE)).shadow_sm()
                        .flex().items_center().justify_center()
                        .child(empty_message("Your print starts here", "Open an image to preview the separated plates")))
                    .child(muted("A4 · 297 × 210 mm · Landscape"))),
            Screen::Simulate => stage
                .child(div().flex().flex_1().min_h_0().p(px(24.0)).items_center().justify_center()
                    .child(column().items_center().gap(px(10.0)).size_full().max_w(px(490.0))
                        .child(muted(if self.shirt_side == 0 { "FRONT" } else { "BACK" }))
                        .child(shirt(self.sleeve == 1))
                        .child(muted("Choose a design from the library to place it on the shirt"))))
                .child(self.library_shelf(cx)),
            Screen::Help => stage.child(
                div().id("help-content").size_full().overflow_y_scroll().p(px(40.0))
                    .child(column().gap(px(24.0)).max_w(px(790.0))
                        .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("A workspace for images and print"))
                        .child(muted("Convert, compose, separate, and preview — all in one place."))
                        .children([
                            ("Convert", "Prepare raster images for vector tracing. Adjust the look, threshold, colours, and detail. Compare the SVG with the original, or use Batch to organise multiple images."),
                            ("Compose", "Arrange graphics on an artboard. The toolbar holds selection, node editing, pen, text, shape, and fill tools. Layers and exact transforms live in the left inspector."),
                            ("Print", "Choose one ink, CMYK, or spot colours. Set levels, halftone or dither options, page dimensions, and registration marks. Review each plate before export."),
                            ("Simulate", "Arrange designs on the front or back of a shirt. Use the size, colour, placement, and measurement controls to prepare a mock-up."),
                            ("Shortcuts", "Select V   ·   Nodes A   ·   Pen P   ·   Text T   ·   Shape S   ·   Fill F"),
                        ].into_iter().map(|(title, detail)| section(title, div().text_sm().text_color(rgb(MUTED)).child(detail))))),
            ),
        }
    }

    fn library_shelf(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap(px(12.0))
            .flex_shrink_0()
            .px(px(20.0))
            .py(px(14.0))
            .border_t_1()
            .border_color(rgb(LINE))
            .bg(rgb(PANEL))
            .child(
                row()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("LIBRARY"),
                    )
                    .child(self.choices(
                        "shelf-source",
                        &["Convert", "Print"],
                        self.library,
                        cx,
                        |s, i| s.library = i,
                    ))
                    .child(div().flex_1())
                    .child(button("+ Add file")),
            )
            .child(muted("No saved graphics yet"))
    }

    fn batch_stage(&self) -> Div {
        column()
            .gap_0()
            .size_full()
            .child(
                row()
                    .px(px(24.0))
                    .py(px(18.0))
                    .bg(rgb(PANEL))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Batch conversion"),
                    )
                    .child(muted("0 images"))
                    .child(div().flex_1())
                    .child(button("Stop"))
                    .child(button("Download ZIP")),
            )
            .child(
                row()
                    .px(px(24.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(div().flex_1().child(muted("IMAGE")))
                    .child(div().w(px(150.0)).child(muted("SETTINGS")))
                    .child(div().w(px(120.0)).child(muted("STATUS"))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(empty_message(
                        "Add images to your batch",
                        "Drop multiple images or choose a folder",
                    )),
            )
    }
}

fn pattern_grid() -> Div {
    column().gap(px(1.0)).children((0..16).map(|_| {
        row().gap(px(1.0)).children((0..16).map(|_| {
            div()
                .size(px(12.0))
                .bg(rgb(WASH))
                .border_1()
                .border_color(rgb(LINE))
        }))
    }))
}

fn artboard() -> Div {
    div().flex_1().min_h_0().w_full().child(
        canvas(
            |_, _, _| (),
            |bounds, (), window, _| {
                let width = bounds.size.width.min(bounds.size.height * (297.0 / 210.0));
                let height = width * (210.0 / 297.0);
                let origin = bounds.origin
                    + point(
                        (bounds.size.width - width) / 2.0,
                        (bounds.size.height - height) / 2.0,
                    );
                let page = gpui::Bounds::new(origin, gpui::size(width, height));
                window.paint_quad(gpui::fill(page, rgb(PANEL)));
                window.paint_quad(gpui::outline(page, rgb(0xd8dce2), Default::default()));
            },
        )
        .size_full(),
    )
}

/// Decorative shirt silhouette; no garment simulation or image processing is performed.
fn shirt(long_sleeve: bool) -> Div {
    div().w_full().flex_1().min_h_0().child(
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let edge = bounds.size.width.min(bounds.size.height);
                let origin = bounds.origin
                    + point(
                        (bounds.size.width - edge) / 2.0,
                        (bounds.size.height - edge) / 2.0,
                    );
                let p = |x: f32, y: f32| origin + point(edge * x, edge * y);
                let outline: &[(f32, f32)] = if long_sleeve {
                    &[
                        (0.38, 0.08),
                        (0.22, 0.15),
                        (0.04, 0.72),
                        (0.16, 0.77),
                        (0.29, 0.39),
                        (0.29, 0.91),
                        (0.71, 0.91),
                        (0.71, 0.39),
                        (0.84, 0.77),
                        (0.96, 0.72),
                        (0.78, 0.15),
                        (0.62, 0.08),
                    ]
                } else {
                    &[
                        (0.38, 0.08),
                        (0.22, 0.15),
                        (0.04, 0.31),
                        (0.17, 0.47),
                        (0.29, 0.37),
                        (0.29, 0.91),
                        (0.71, 0.91),
                        (0.71, 0.37),
                        (0.83, 0.47),
                        (0.96, 0.31),
                        (0.78, 0.15),
                        (0.62, 0.08),
                    ]
                };
                for fill in [true, false] {
                    let mut path = if fill {
                        PathBuilder::fill()
                    } else {
                        PathBuilder::stroke(px(1.5))
                    };
                    path.move_to(p(outline[0].0, outline[0].1));
                    for &(x, y) in &outline[1..] {
                        path.line_to(p(x, y));
                    }
                    path.curve_to(p(0.38, 0.08), p(0.5, 0.27));
                    path.close();
                    if let Ok(path) = path.build() {
                        window.paint_path(path, rgb(if fill { 0xfefefe } else { 0xb6becb }));
                    }
                }
                let mut seams = PathBuilder::stroke(px(1.0));
                seams.move_to(p(0.31, 0.87));
                seams.line_to(p(0.69, 0.87));
                seams.move_to(p(0.36, 0.095));
                seams.curve_to(p(0.64, 0.095), p(0.5, 0.29));
                if let Ok(path) = seams.build() {
                    window.paint_path(path, rgb(0xd6dce4));
                }
            },
        )
        .size_full(),
    )
}
