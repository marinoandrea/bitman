//! GPUI integration for garment mock-ups. Background rendering is coalesced;
//! imports and exports capture their own artwork, side, and scene snapshots.
use crate::{
    app::{Screen, Workspace},
    controls::*,
};
use bitman::{
    effects::{self, Engine, ImageView, Operation},
    simulation::{Artwork, CANVAS, Design, Origin, PREVIEW_WIDTH, Simulation, ViewTransform},
};
use gpui::{
    Bounds, Context, Div, Image, ImageFormat, MouseButton, MouseMoveEvent, ObjectFit,
    PathPromptOptions, Pixels, Point, RenderImage, Styled, canvas, div, img, prelude::*, px, rgb,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

#[derive(Clone, Copy)]
pub enum Placement {
    Width,
    Height,
    Top,
    X,
    Rotation,
}
impl Placement {
    const ALL: [Self; 5] = [
        Self::Width,
        Self::Height,
        Self::Top,
        Self::X,
        Self::Rotation,
    ];
    fn spec(self, d: &Design) -> (&'static str, f32, f32, f32, f32, &'static str) {
        match self {
            Self::Width => ("Width", d.width, 0.5, 100., 0.5, " cm"),
            Self::Height => ("Height", d.height, 0.5, 100., 0.5, " cm"),
            Self::Top => ("From collar", d.top, -20., 90., 0.5, " cm"),
            Self::X => ("From centre", d.x, -70., 70., 0.5, " cm"),
            Self::Rotation => ("Rotation", d.rotation, -180., 180., 1., "°"),
        }
    }
    fn set(self, d: &mut Design, v: f32) {
        match self {
            Self::Width => d.set_width(v),
            Self::Height => d.set_height(v),
            Self::Top => d.top = v.clamp(-20., 90.),
            Self::X => d.x = v.clamp(-70., 70.),
            Self::Rotation => d.rotation = v.clamp(-180., 180.),
        }
    }
}

#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize,
    Rotate,
}
struct Drag {
    kind: DragKind,
    start: [f32; 2],
    original: Design,
}
struct LibraryItem {
    artwork: Arc<Artwork>,
    thumbnail: Arc<RenderImage>,
}

pub struct SimulationUi {
    pub scene: Simulation,
    library: Vec<LibraryItem>,
    pub filter: usize,
    pub preview: Option<Arc<RenderImage>>,
    pub status: String,
    pub actual_size: bool,
    pub exporting: bool,
    imports: usize,
    directory: PathBuf,
    rendering: bool,
    revision: u64,
    viewport: Rc<RefCell<Bounds<Pixels>>>,
    slider_bounds: Rc<RefCell<[Bounds<Pixels>; 5]>>,
    pub active_slider: Option<Placement>,
    drag: Option<Drag>,
}
impl Default for SimulationUi {
    fn default() -> Self {
        Self {
            scene: Simulation::default(),
            library: Vec::new(),
            filter: 0,
            preview: None,
            status: "Add a file or send artwork from Convert / Print".into(),
            actual_size: false,
            exporting: false,
            imports: 0,
            directory: PathBuf::from("."),
            rendering: false,
            revision: 0,
            viewport: Default::default(),
            slider_bounds: Rc::new(RefCell::new([Bounds::default(); 5])),
            active_slider: None,
            drag: None,
        }
    }
}

fn render_image(mut pixels: image::RgbaImage) -> Arc<RenderImage> {
    // The mock-up is opaque; GPUI expects straight BGRA, not RGBA.
    for p in pixels.pixels_mut() {
        p.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]))
}
fn thumbnail(art: &Artwork) -> Arc<RenderImage> {
    let mut rgba = image::RgbaImage::from_raw(
        art.pixels.width(),
        art.pixels.height(),
        art.pixels.data().to_vec(),
    )
    .unwrap();
    for p in rgba.pixels_mut() {
        if p[3] != 0 {
            for k in 0..3 {
                p[k] = ((u32::from(p[k]) * 255 + u32::from(p[3]) / 2) / u32::from(p[3])).min(255)
                    as u8;
            }
        }
    }
    render_image(image::imageops::thumbnail(&rgba, 72, 56))
}

impl Workspace {
    pub fn simulation_changed(&mut self, cx: &mut Context<Self>) {
        self.simulation.revision += 1;
        self.render_simulation(cx);
        cx.notify();
    }
    pub fn render_simulation(&mut self, cx: &mut Context<Self>) {
        if self.simulation.rendering {
            return;
        }
        self.simulation.rendering = true;
        let revision = self.simulation.revision;
        let scene = self.simulation.scene.clone();
        let side = scene.back;
        let task = cx.background_executor().spawn(async move {
            let pixmap = scene.render(PREVIEW_WIDTH, scene.measurements, true)?;
            let rgba = image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
                .ok_or("Invalid mock-up pixels")?;
            Ok::<_, String>(render_image(rgba))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.simulation.rendering = false;
                // Publish completed frames while dragging, then render the latest state.
                match result {
                    Ok(image) if side == this.simulation.scene.back => {
                        if let Some(old) = this.simulation.preview.replace(image) {
                            this.editor.retired.push(old);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => this.simulation.status = format!("Preview failed: {e}"),
                }
                if revision != this.simulation.revision {
                    this.render_simulation(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn open_simulation_file(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add artwork to the shirt".into()),
        });
        cx.spawn(async move |this, cx| match prompt.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    for path in paths {
                        this.load_simulation_path(path, cx);
                    }
                });
            }
            Ok(Err(e)) => {
                let _ = this.update(cx, |this, cx| {
                    this.simulation.status = format!("Could not open dialog: {e}");
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    pub fn load_simulation_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(parent) = path.parent() {
            self.simulation.directory = parent.to_owned();
        }
        let side = self.simulation.scene.back;
        self.simulation.imports += 1;
        self.simulation.status = "Loading artwork…".into();
        let task = cx.background_executor().spawn(async move {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let mut reader = image::ImageReader::open(&path)
                .map_err(|e| e.to_string())?
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(512 * 1024 * 1024);
            reader.limits(limits);
            let image = reader
                .decode()
                .map_err(|e| format!("{name}: {e}"))?
                .thumbnail(2048, 2048)
                .to_rgba8();
            let (width, height) = image.dimensions();
            let mut bytes = image.into_raw();
            for p in bytes.as_chunks_mut::<4>().0.iter_mut() {
                p.swap(0, 2);
            }
            Artwork::from_bgra(name, Origin::File, width, height, &bytes)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_simulation_import(result, side, cx)
            });
        })
        .detach();
        cx.notify();
    }
    fn finish_simulation_import(
        &mut self,
        result: Result<Artwork, String>,
        side: bool,
        cx: &mut Context<Self>,
    ) {
        self.simulation.imports = self.simulation.imports.saturating_sub(1);
        match result {
            Ok(art) => {
                self.end_simulation_drag();
                self.simulation.status = format!(
                    "Added {} to {} · drag to move; blue handles resize / rotate",
                    art.name,
                    if side { "back" } else { "front" }
                );
                let thumb = thumbnail(&art);
                let art = Arc::new(art);
                self.simulation.library.push(LibraryItem {
                    artwork: art.clone(),
                    thumbnail: thumb,
                });
                self.add_simulation_artwork(art, side);
                self.simulation_changed(cx);
            }
            Err(e) => {
                self.simulation.status = format!("Could not add artwork: {e}");
                cx.notify();
            }
        }
    }
    fn add_simulation_artwork(&mut self, art: Arc<Artwork>, side: bool) {
        let scene = &mut self.simulation.scene;
        let previous_side = scene.back;
        let selection = scene.selected;
        scene.back = side;
        scene.add(art);
        if previous_side != side {
            scene.back = previous_side;
            scene.selected = selection;
        }
    }
    pub fn send_to_simulate(&mut self, cx: &mut Context<Self>) {
        if !self.can_export_svg() {
            return;
        }
        let Some(source) = self.editor.source.clone() else {
            return;
        };
        if let Some(parent) = self.editor.path.as_ref().and_then(|p| p.parent()) {
            self.simulation.directory = parent.to_owned();
        }
        let mut params = self.editor.params;
        params.operation = if self.screen == Screen::Print {
            Operation::Print
        } else {
            Operation::Convert
        };
        params.max_side = 2048;
        let origin = if params.operation == Operation::Print {
            Origin::Print
        } else {
            Origin::Convert
        };
        let name = format!(
            "{} ({})",
            self.editor
                .path
                .as_ref()
                .and_then(|p| p.file_stem())
                .unwrap_or_default()
                .to_string_lossy(),
            if origin == Origin::Print {
                "Print"
            } else {
                "Convert"
            }
        );
        let renderer = cx.svg_renderer();
        let side = self.simulation.scene.back;
        self.screen = Screen::Simulate;
        self.simulation.imports += 1;
        self.simulation.status = "Preparing artwork snapshot…".into();
        self.render_simulation(cx);
        let task = cx.background_executor().spawn(async move {
            let size = source.size(0);
            let bytes = source.as_bytes(0).ok_or("No source pixels")?;
            let view = ImageView::new(
                bytes,
                size.width.0 as usize,
                size.height.0 as usize,
                size.width.0 as usize * 4,
            )
            .map_err(|e| e.to_string())?;
            let mut engine = Engine::default();
            engine.render(view, params).map_err(|e| e.to_string())?;
            if params.operation == Operation::Print {
                engine.composite_artwork(params);
                Artwork::from_bgra(
                    name,
                    origin,
                    engine.info.width as u32,
                    engine.info.height as u32,
                    engine.pixels.as_flattened(),
                )
            } else {
                // Follow the Convert preview's SVG rasterization and alpha correction.
                params.upscale = 1.0;
                engine.write_svg(params, true);
                let raster = Image::from_bytes(ImageFormat::Svg, engine.svg.into_bytes())
                    .to_image_data(renderer)
                    .map_err(|e| e.to_string())?;
                let size = raster.size(0);
                let bytes = raster.as_bytes(0).ok_or("No SVG pixels")?;
                let bgra: Vec<u8> = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| effects::color::premultiplied_rgba_to_bgra(*p))
                    .collect();
                Artwork::from_bgra(
                    name,
                    origin,
                    size.width.0 as u32,
                    size.height.0 as u32,
                    &bgra,
                )
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_simulation_import(result, side, cx)
            });
        })
        .detach();
        cx.notify();
    }
    pub fn remove_simulation_design(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::Simulate || self.simulation.scene.selection().is_none() {
            return;
        }
        self.end_simulation_drag();
        self.simulation.scene.remove();
        self.simulation.status = "Removed placement · artwork remains in the library".into();
        self.simulation_changed(cx);
    }
    pub fn can_export_mockup(&self) -> bool {
        !self.simulation.exporting
            && self.simulation.imports == 0
            && !self.simulation.scene.current().is_empty()
    }
    pub fn save_mockup(&mut self, cx: &mut Context<Self>) {
        if !self.can_export_mockup() {
            return;
        }
        // Freeze the scene when Save is pressed, even if the user edits while the dialog is open.
        let scene = self.simulation.scene.clone();
        let name = if scene.back {
            "mockup-back.jpg"
        } else {
            "mockup-front.jpg"
        };
        let prompt = cx.prompt_for_new_path(&self.simulation.directory, Some(name));
        self.simulation.exporting = true;
        self.simulation.status = "Choose where to save the mock-up…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(path))) => path,
                result => {
                    let _ = this.update(cx, |this, cx| {
                        this.simulation.exporting = false;
                        this.simulation.status = match result {
                            Ok(Err(e)) => format!("Export dialog failed: {e}"),
                            _ => "Export cancelled".into(),
                        };
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(parent) = path.parent() {
                    this.simulation.directory = parent.to_owned();
                }
                this.simulation.status = "Exporting 3000 × 2100 JPG…".into();
                cx.notify();
            });
            let task = cx
                .background_executor()
                .spawn(async move { scene.save_jpeg(&path) });
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.simulation.exporting = false;
                this.simulation.status = match result {
                    Ok(path) => format!("Saved {}", path.display()),
                    Err(e) => format!("Export failed: {e}"),
                };
                cx.notify();
            });
        })
        .detach();
    }
    fn simulation_toggle(
        &self,
        title: &'static str,
        value: bool,
        change: fn(&mut Simulation),
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        check(title, value)
            .id(title)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                change(&mut this.simulation.scene);
                this.simulation_changed(cx);
            }))
    }
    fn placement_control(&self, key: Placement, d: &Design, cx: &mut Context<Self>) -> Div {
        let (title, v, min, max, step, unit) = key.spec(d);
        let bounds = self.simulation.slider_bounds.clone();
        column()
            .gap(px(3.0))
            .child(
                slider(
                    title,
                    format!("{v:.1}{unit}"),
                    ((v - min) / (max - min)).clamp(0.0, 1.0),
                )
                .id(("placement-slider", key as usize))
                .debug_selector(|| title.into())
                .relative()
                .cursor_pointer()
                .child(
                    canvas(
                        move |rect, _, _| bounds.borrow_mut()[key as usize] = rect,
                        |_, (), _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        this.simulation.active_slider = Some(key);
                        this.set_placement_slider(key, event.position, cx);
                    }),
                ),
            )
            .child(
                row()
                    .justify_end()
                    .children([-1.0_f32, 1.0].into_iter().map(|direction| {
                        button(if direction < 0.0 { "−" } else { "+" })
                            .id((
                                if direction < 0.0 {
                                    "placement-minus"
                                } else {
                                    "placement-plus"
                                },
                                key as usize,
                            ))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(d) = this.simulation.scene.selection_mut() {
                                    let (_, value, _, _, _, _) = key.spec(d);
                                    key.set(d, (value + direction * step).clamp(min, max));
                                }
                                this.simulation_changed(cx);
                            }))
                    })),
            )
    }
    fn set_placement_slider(
        &mut self,
        key: Placement,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let bounds = self.simulation.slider_bounds.borrow()[key as usize];
        if bounds.size.width <= px(0.0) {
            return;
        }
        if let Some(d) = self.simulation.scene.selection_mut() {
            let (_, _, min, max, step, _) = key.spec(d);
            let t = ((position.x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0);
            key.set(d, ((min + t * (max - min)) / step).round() * step);
            self.simulation_changed(cx);
        }
    }
    pub fn simulate_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let scene = &self.simulation.scene;
        let [w, h] = scene.dimensions();
        let mut side = column()
            .gap_0()
            .child(section(
                "Shirt",
                column()
                    .child(label("Shirt size"))
                    .child(self.choices(
                        "shirt-size",
                        &["XS", "S", "M", "L", "XL", "XXL"],
                        scene.size,
                        cx,
                        |s, i| s.simulation.scene.size = i,
                    ))
                    .child(muted(format!(
                        "Approx. {w:.0} × {h:.0} cm · width × length"
                    )))
                    .child(muted("Illustrative sizes; check your actual garment."))
                    .child(self.choices(
                        "sleeve",
                        &["Short sleeve", "Long sleeve"],
                        usize::from(scene.long_sleeve),
                        cx,
                        |s, i| s.simulation.scene.long_sleeve = i == 1,
                    ))
                    .child(self.choices(
                        "shirt-side",
                        &["Front", "Back"],
                        usize::from(scene.back),
                        cx,
                        |s, i| s.simulation.scene.set_side(i == 1),
                    )),
            ))
            .child(section(
                "Shirt colour",
                column()
                    .child(
                        row().gap(px(6.0)).children(
                            [
                                0xffffff, 0x22252b, 0x315ed8, 0xd44a54, 0xe9bc46, 0x49866c,
                                0xb49bc8,
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(i, c)| {
                                let color = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
                                div()
                                    .id(("shirt-color", i))
                                    .size(px(25.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(c))
                                    .border_2()
                                    .border_color(rgb(if scene.color == color {
                                        ACCENT
                                    } else {
                                        LINE
                                    }))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.simulation.scene.color = color;
                                        this.simulation_changed(cx);
                                    }))
                            }),
                        ),
                    )
                    .child(
                        button("Use Print colour")
                            .id("use-print-color")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.simulation.scene.color = if this.editor.params.dark {
                                    this.editor.params.dark_bg
                                } else {
                                    [255; 3]
                                };
                                this.simulation_changed(cx);
                            })),
                    ),
            ))
            .child(section(
                "Designs",
                column()
                    .when(scene.current().is_empty(), |c| {
                        c.child(muted("Add artwork using the library below."))
                    })
                    .children(scene.current().iter().enumerate().map(|(i, d)| {
                        div()
                            .id(("placed-design", i))
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .text_xs()
                            .bg(rgb(if scene.selected == Some(i) {
                                BUTTON
                            } else {
                                PANEL
                            }))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .w_full()
                            .min_w_0()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(format!("{} · {}", i + 1, d.artwork.name)),
                            )
                            .child(
                                button("Remove")
                                    .id(("remove-placed-design", i))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.simulation.scene.selected = Some(i);
                                        this.remove_simulation_design(cx);
                                    })),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.simulation.scene.selected = Some(i);
                                this.simulation_changed(cx);
                            }))
                    })),
            ));
        if let Some(d) = scene.selection() {
            side = side.child(section(
                "Placement",
                column()
                    .children(
                        Placement::ALL
                            .into_iter()
                            .map(|key| self.placement_control(key, d, cx)),
                    )
                    .child(self.simulation_toggle(
                        "Keep proportions",
                        d.keep_proportions,
                        |s| {
                            if let Some(d) = s.selection_mut() {
                                d.keep_proportions = !d.keep_proportions;
                            }
                        },
                        cx,
                    ))
                    .child(
                        row().children(
                            ["Centre", "Chest", "Full front"]
                                .into_iter()
                                .enumerate()
                                .map(|(i, name)| {
                                    button(name).id(name).cursor_pointer().on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.simulation.scene.preset(i);
                                            this.simulation_changed(cx);
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(
                        row()
                            .child(
                                button("Duplicate")
                                    .id("duplicate-design")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.simulation.scene.duplicate();
                                        this.simulation_changed(cx);
                                    })),
                            )
                            .child(
                                button("Remove")
                                    .id("remove-design")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.remove_simulation_design(cx);
                                    })),
                            ),
                    ),
            ));
        }
        side.child(section(
            "View & export",
            column()
                .child(self.simulation_toggle(
                    "Show shirt measurements",
                    scene.measurements,
                    |s| s.measurements = !s.measurements,
                    cx,
                ))
                .child(self.simulation_toggle(
                    "Crop designs to the shirt",
                    scene.crop,
                    |s| s.crop = !s.crop,
                    cx,
                ))
                .child(self.simulation_toggle(
                    "Include measurements in JPG",
                    scene.export_measurements,
                    |s| s.export_measurements = !s.export_measurements,
                    cx,
                ))
                .child(
                    primary_button(if self.simulation.exporting {
                        "Saving…"
                    } else {
                        "Save mock-up as JPG"
                    })
                    .id("save-mockup")
                    .when(self.can_export_mockup(), |b| {
                        b.cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.save_mockup(cx)))
                    })
                    .when(!self.can_export_mockup(), |b| b.opacity(0.45)),
                )
                .child(muted("Exports this side · 3000 × 2100 pixels")),
        ))
    }
    pub fn simulation_stage(&self, cx: &mut Context<Self>) -> Div {
        let bounds = self.simulation.viewport.clone();
        let actual = self.simulation.actual_size;
        let mut view = div()
            .relative()
            .when(actual, |v| {
                v.w(px(PREVIEW_WIDTH as f32))
                    .h(px(PREVIEW_WIDTH as f32 * CANVAS[1] / CANVAS[0]))
                    .mx_auto()
                    .flex_shrink_0()
            })
            .when(!actual, |v| v.size_full().min_w_0().min_h_0());
        if let Some(image) = &self.simulation.preview {
            view = view.child(
                img(image.clone())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            );
        }
        view = view
            .child(
                canvas(
                    move |rect, _, _| *bounds.borrow_mut() = rect,
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.start_simulation_drag(event.position, cx)
                }),
            );
        column()
            .gap_0()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(
                row()
                    .justify_between()
                    .px(px(20.0))
                    .py(px(8.0))
                    .child(label(if self.simulation.scene.back {
                        "BACK"
                    } else {
                        "FRONT"
                    }))
                    .child(muted(
                        "Drag artwork · lower handle resizes · upper handle rotates",
                    )),
            )
            .child(
                div()
                    .id("simulation-viewport")
                    .flex_1()
                    .h(px(0.0))
                    .min_h_0()
                    .min_w_0()
                    .overflow_scroll()
                    .child(view),
            )
            .child(self.simulation_library(cx))
    }
    fn simulation_library(&self, cx: &mut Context<Self>) -> Div {
        column().gap(px(8.0)).flex_shrink_0().px(px(20.0)).py(px(12.0)).bg(rgb(PANEL)).border_t_1().border_color(rgb(LINE))
            .child(row().child(heading("LIBRARY").text_xs()).child(self.choices("simulation-library", &["All", "Convert", "Print", "Files"], self.simulation.filter, cx, |s, i| s.simulation.filter = i)).child(div().flex_1()).child(button("+ Add file").id("add-simulation-file").cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.open_simulation_file(cx)))))
            .child(div().id("simulation-library-items").overflow_x_scroll().child(row().min_h(px(65.0)).when(self.simulation.library.is_empty(), |r| r.child(muted("PNG · JPG · GIF · WebP · BMP · TIFF, or Add to Simulate from Convert / Print"))).children(self.simulation.library.iter().enumerate().filter(|(_, item)| match self.simulation.filter { 1 => item.artwork.origin == Origin::Convert, 2 => item.artwork.origin == Origin::Print, 3 => item.artwork.origin == Origin::File, _ => true }).map(|(i, item)| {
                column().id(("library-artwork", i)).flex_shrink_0().w(px(120.0)).p(px(5.0)).rounded(px(4.0)).bg(rgb(BUTTON)).cursor_pointer().child(img(item.thumbnail.clone()).h(px(45.0)).w_full().object_fit(ObjectFit::Contain)).child(div().text_xs().overflow_hidden().whitespace_nowrap().child(item.artwork.name.clone())).on_click(cx.listener(move |this, _, _, cx| { let art = this.simulation.library[i].artwork.clone(); this.simulation.scene.add(art); this.simulation_changed(cx); }))
            }))))
    }
    fn simulation_transform(&self) -> Option<ViewTransform> {
        let rect = *self.simulation.viewport.borrow();
        ViewTransform::fit(
            [rect.origin.x.into(), rect.origin.y.into()],
            [rect.size.width.into(), rect.size.height.into()],
        )
    }
    fn start_simulation_drag(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(transform) = self.simulation_transform() else {
            return;
        };
        let point = transform.world([point.x.into(), point.y.into()]);
        let scene = &mut self.simulation.scene;
        let mut kind = DragKind::Move;
        let mut handle = false;
        if let Some(d) = scene.selection() {
            let p = d.local(point, scene.collar());
            let radius = (9.0 / transform.scale).max(1.0);
            if (p[0] - d.width / 2.0).hypot(p[1] - d.height / 2.0) < radius {
                kind = DragKind::Resize;
                handle = true;
            } else if p[0].hypot(p[1] + d.height / 2.0 + 4.0) < radius {
                kind = DragKind::Rotate;
                handle = true;
            }
        }
        if !handle {
            scene.selected = scene.hit(point);
        }
        self.simulation.drag = scene.selection().cloned().map(|original| Drag {
            kind,
            start: point,
            original,
        });
        self.simulation_changed(cx);
    }
    pub fn move_simulation_pointer(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.screen != Screen::Simulate || event.pressed_button != Some(MouseButton::Left) {
            self.end_simulation_drag();
            return;
        }
        if let Some(key) = self.simulation.active_slider {
            self.set_placement_slider(key, event.position, cx);
            return;
        }
        let Some(transform) = self.simulation_transform() else {
            return;
        };
        let point = transform.world([event.position.x.into(), event.position.y.into()]);
        let Some(drag) = &self.simulation.drag else {
            return;
        };
        let collar = self.simulation.scene.collar();
        let mut d = drag.original.clone();
        match drag.kind {
            DragKind::Move => {
                d.x = (d.x + point[0] - drag.start[0]).clamp(-70.0, 70.0);
                d.top = (d.top + point[1] - drag.start[1]).clamp(-20.0, 90.0);
            }
            DragKind::Resize => {
                let p = d.local(point, collar);
                let centre = d.centre(collar);
                d.set_width((p[0] * 2.0).max(0.5));
                if !d.keep_proportions {
                    d.set_height((p[1] * 2.0).max(0.5));
                }
                d.top = centre[1] - collar - d.height / 2.0;
            }
            DragKind::Rotate => {
                let c = d.centre(collar);
                let a = (point[1] - c[1]).atan2(point[0] - c[0]);
                let b = (drag.start[1] - c[1]).atan2(drag.start[0] - c[0]);
                d.rotation = (d.rotation + (a - b).to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
            }
        }
        if let Some(selected) = self.simulation.scene.selection_mut() {
            *selected = d;
        }
        self.simulation_changed(cx);
    }
    pub fn end_simulation_drag(&mut self) {
        self.simulation.drag = None;
        self.simulation.active_slider = None;
    }
}
