//! Native composition controls and asynchronous SVG rendering.
use crate::{
    app::{Screen, Workspace},
    controls::*,
};
use bitman::{
    composition::{self, Artwork, Content, Document, Object, Origin, Shape, ViewTransform},
    effects::{self, Engine, ImageView, Operation},
};
use gpui::{
    Bounds, Context, Div, Image, ImageFormat, MouseButton, MouseDownEvent, MouseMoveEvent,
    ObjectFit, PathBuilder, PathPromptOptions, Pixels, RenderImage, Styled, canvas, div, img,
    point, prelude::*, px, rgb,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Property {
    PageWidth,
    PageHeight,
    X,
    Y,
    Width,
    Height,
    Rotation,
    StrokeWidth,
    Text,
    Fill,
    Stroke,
}
#[derive(Clone)]
pub struct Edit {
    pub property: Property,
    pub value: String,
    pub selection: std::ops::Range<usize>,
    pub marked: Option<std::ops::Range<usize>>,
}
#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize,
    Rotate,
    Node(usize),
    Create,
}
struct Drag {
    kind: DragKind,
    start: [f32; 2],
    originals: Vec<Object>,
    checkpointed: bool,
    frame: Option<Object>,
}
struct LibraryItem {
    art: Arc<Artwork>,
    thumbnail: Arc<RenderImage>,
}
pub struct ComposeUi {
    pub document: Document,
    pub status: String,
    pub actual_size: bool,
    pub exporting: bool,
    pub edit: Option<Edit>,
    pub input_bounds: Rc<RefCell<Bounds<Pixels>>>,
    pub shape: usize,
    pub colour_target: usize,
    pub fill: Option<u32>,
    pub stroke: Option<u32>,
    pub stroke_width: f32,
    pub pending: Vec<[f32; 2]>,
    pub node: Option<usize>,
    library: Vec<LibraryItem>,
    filter: usize,
    directory: PathBuf,
    imports: usize,
    preview: Option<Arc<RenderImage>>,
    viewport: Rc<RefCell<Bounds<Pixels>>>,
    rendering: bool,
    revision: u64,
    drag: Option<Drag>,
}
impl Default for ComposeUi {
    fn default() -> Self {
        Self {
            document: Document::default(),
            status: "Choose Shape and drag on the page, or add artwork".into(),
            actual_size: false,
            exporting: false,
            edit: None,
            input_bounds: Default::default(),
            shape: 0,
            colour_target: 0,
            fill: Some(0x222222),
            stroke: None,
            stroke_width: 0.5,
            pending: Vec::new(),
            node: None,
            library: Vec::new(),
            filter: 0,
            directory: PathBuf::from("."),
            imports: 0,
            preview: None,
            viewport: Default::default(),
            rendering: false,
            revision: 0,
            drag: None,
        }
    }
}
fn raster(
    svg: &str,
    size: [f32; 2],
    side: f32,
    decode: impl FnOnce(Image) -> Result<Arc<RenderImage>, String>,
) -> Result<Arc<RenderImage>, String> {
    let scale = side / size[0].max(size[1]);
    let width = (size[0] * scale).round().max(1.) as u32;
    let height = (size[1] * scale).round().max(1.) as u32;
    // Only generated documents enter here. Replace the root's physical dimensions
    // with bounded raster dimensions while preserving its viewBox and contents.
    let body = svg.split_once('>').ok_or("Invalid SVG")?.1;
    let bounded = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {} {}\">{body}",
        size[0], size[1]
    );
    let image = decode(Image::from_bytes(ImageFormat::Svg, bounded.into_bytes()))?;
    let bytes = image.as_bytes(0).ok_or("No SVG pixels")?;
    let bgra: Vec<u8> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| effects::color::premultiplied_rgba_to_bgra(*p))
        .collect();
    let size = image.size(0);
    let rgba = image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, bgra)
        .ok_or("Invalid composition pixels")?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

impl Workspace {
    pub fn compose_changed(&mut self, cx: &mut Context<Self>) {
        self.compose.revision += 1;
        self.render_compose(cx);
        cx.notify();
    }
    pub fn render_compose(&mut self, cx: &mut Context<Self>) {
        if self.compose.rendering {
            return;
        }
        self.compose.rendering = true;
        let revision = self.compose.revision;
        let page = self.compose.document.page.clone();
        let renderer = cx.svg_renderer();
        let task = cx.background_executor().spawn(async move {
            raster(&page.svg(), page.size, 1400., |image| {
                image.to_image_data(renderer).map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.compose.rendering = false;
                match result {
                    Ok(image) => {
                        if let Some(old) = this.compose.preview.replace(image) {
                            this.editor.retired.push(old);
                        }
                    }
                    Err(e) => this.compose.status = format!("Preview failed: {e}"),
                }
                if revision != this.compose.revision {
                    this.render_compose(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn compose_edit_page(
        &mut self,
        edit: impl FnOnce(&mut composition::Page),
        cx: &mut Context<Self>,
    ) {
        if !self.commit_compose_input(cx) {
            return;
        }
        self.end_compose_drag();
        self.compose.document.edit(edit);
        self.compose_changed(cx);
    }
    pub fn compose_undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        self.end_compose_drag();
        if !self.commit_compose_input(cx) {
            return;
        }
        self.compose.pending.clear();
        self.compose.node = None;
        if redo {
            self.compose.document.redo();
        } else {
            self.compose.document.undo();
        }
        self.compose_changed(cx);
    }
    pub fn remove_compose_selection(&mut self, cx: &mut Context<Self>) {
        if self.compose.edit.is_some() {
            self.compose_input_delete(false, cx);
            return;
        }
        if self.compose_tool == 1
            && let Some(node) = self.compose.node
        {
            let selected = self.compose.document.page.selection().map(|o| o.id);
            self.compose_edit_page(
                |page| {
                    page.edit_selected(|o| {
                        if Some(o.id) != selected {
                            return;
                        }
                        if !matches!(o.content, Content::Path { .. })
                            && let Some(points) = o.vertices()
                        {
                            o.content = Content::Path {
                                points,
                                closed: true,
                            };
                        }
                        if let Content::Path { points, closed } = &mut o.content
                            && points.len() > if *closed { 3 } else { 2 }
                            && node < points.len()
                        {
                            points.remove(node);
                            o.normalize_path();
                        }
                    })
                },
                cx,
            );
            self.compose.node = None;
        } else {
            self.compose_edit_page(|p| p.remove(), cx);
        }
    }
    pub fn compose_finish_path(&mut self, closed: bool, cx: &mut Context<Self>) {
        if self.compose.pending.len() < if closed { 3 } else { 2 } {
            self.compose.status = "A path needs two points; a closed path needs three".into();
            cx.notify();
            return;
        }
        let points = std::mem::take(&mut self.compose.pending);
        let min: [f32; 2] =
            std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min));
        let max: [f32; 2] = std::array::from_fn(|k| {
            points
                .iter()
                .map(|p| p[k])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        let size: [f32; 2] = std::array::from_fn(|k| (max[k] - min[k]).max(0.1));
        let points = points
            .iter()
            .map(|p| std::array::from_fn(|k| (p[k] - min[k]) / size[k]))
            .collect();
        let mut object = Object::new(
            Content::Path { points, closed },
            std::array::from_fn(|k| min[k] + size[k] / 2.),
            size,
        );
        object.fill = if closed { self.compose.fill } else { None };
        object.stroke = if closed {
            self.compose.stroke
        } else {
            Some(
                self.compose
                    .stroke
                    .or(self.compose.fill)
                    .unwrap_or(0x222222),
            )
        };
        object.stroke_width = self.compose.stroke_width;
        self.compose_edit_page(
            |p| {
                p.add(object);
            },
            cx,
        );
        self.compose_tool = 0;
    }
    pub fn compose_tool_changed(&mut self, cx: &mut Context<Self>) {
        self.end_compose_drag();
        self.commit_compose_input(cx);
        self.compose.node = None;
        if self.compose_tool != 2 {
            self.compose.pending.clear();
        }
        self.compose.status = match self.compose_tool {
            1 => "Select a polygonal shape or path, then drag its nodes · Delete removes a node",
            2 => "Click to add path points · Enter finishes · click the first point to close · Esc cancels",
            3 => "Click the page to add text · edit its content in the inspector",
            4 => "Drag on the page to draw the chosen shape",
            5 => "Choose a fill colour, then click an object",
            _ => "Click to select · Shift-click adds to selection · drag to move · blue handles resize / rotate",
        }.into();
        cx.notify();
    }
    fn compose_apply_colour(&mut self, value: Option<u32>, cx: &mut Context<Self>) {
        let stroke = self.compose.colour_target == 1;
        if stroke {
            self.compose.stroke = value;
        } else {
            self.compose.fill = value;
        }
        self.compose_edit_page(
            |p| {
                p.edit_selected(|o| {
                    if stroke {
                        o.stroke = value
                    } else {
                        o.fill = value
                    }
                })
            },
            cx,
        );
    }
    pub fn compose_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let page = &self.compose.document.page;
        let selected = page.selection();
        let mut tools = column().gap_0();
        if self.compose_tool == 4 {
            tools = tools.child(section(
                "Shape",
                self.choices(
                    "compose-shape",
                    &["Rectangle", "Ellipse", "Polygon", "Star"],
                    self.compose.shape,
                    cx,
                    |s, i| s.compose.shape = i,
                ),
            ));
        }
        if self.compose_tool == 2 {
            tools = tools.child(section(
                "Path",
                column()
                    .child(muted(format!(
                        "{} points · straight segments",
                        self.compose.pending.len()
                    )))
                    .child(self.compose_button(
                        "Finish open path",
                        "compose-finish-path",
                        self.compose.pending.len() > 1,
                        cx,
                        |s, cx| s.compose_finish_path(false, cx),
                    ))
                    .child(self.compose_button(
                        "Close path",
                        "compose-close-path",
                        self.compose.pending.len() > 2,
                        cx,
                        |s, cx| s.compose_finish_path(true, cx),
                    )),
            ));
        }
        if let Some(Object {
            content: Content::Text(text),
            ..
        }) = selected
        {
            tools = tools.child(section(
                "Text",
                column()
                    .child(self.compose_field("Content", Property::Text, text.clone(), cx))
                    .child(muted("Monospace · resize using width and height")),
            ));
        }
        let mut side = tools
            .child(section(
                "Artboard",
                column()
                    .child(label("Page preset"))
                    .child(
                        row().children(
                            [
                                ("A5", [210., 148.]),
                                ("A4", [297., 210.]),
                                ("A3", [420., 297.]),
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(i, (title, size))| {
                                button(title)
                                    .id(("compose-preset", i))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.compose_edit_page(
                                            |p| {
                                                p.size = if p.size[0] >= p.size[1] {
                                                    size
                                                } else {
                                                    [size[1], size[0]]
                                                };
                                            },
                                            cx,
                                        );
                                    }))
                            }),
                        ),
                    )
                    .child(
                        row().children(["Landscape", "Portrait"].into_iter().enumerate().map(
                            |(i, title)| {
                                button(title)
                                    .id(("compose-orientation", i))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.compose_edit_page(
                                            |p| {
                                                if (p.size[0] < p.size[1]) != (i == 1) {
                                                    p.size.swap(0, 1);
                                                }
                                            },
                                            cx,
                                        )
                                    }))
                            },
                        )),
                    )
                    .child(self.compose_field(
                        "Width (mm)",
                        Property::PageWidth,
                        format!("{:.1}", page.size[0]),
                        cx,
                    ))
                    .child(self.compose_field(
                        "Height (mm)",
                        Property::PageHeight,
                        format!("{:.1}", page.size[1]),
                        cx,
                    )),
            ))
            .child(section(
                "Layers",
                column()
                    .child(muted("Shift-click to select multiple objects"))
                    .when(page.objects.is_empty(), |c| {
                        c.child(muted("Your page is empty"))
                    })
                    .child(
                        div()
                            .id("compose-layers")
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .children(page.objects.iter().rev().map(|o| {
                                let id = o.id;
                                div()
                                    .id(("compose-layer", id))
                                    .px(px(8.))
                                    .py(px(6.))
                                    .text_xs()
                                    .bg(rgb(if page.selected.contains(&id) {
                                        BUTTON
                                    } else {
                                        PANEL
                                    }))
                                    .cursor_pointer()
                                    .child(format!(
                                        "{}{}",
                                        if o.group.is_some() { "▣ " } else { "" },
                                        o.name
                                    ))
                                    .on_click(cx.listener(
                                        move |this, event: &gpui::ClickEvent, _, cx| {
                                            this.end_compose_drag();
                                            if !this.commit_compose_input(cx) {
                                                return;
                                            }
                                            this.compose.node = None;
                                            this.compose
                                                .document
                                                .page
                                                .select(Some(id), event.modifiers().shift);
                                            cx.notify();
                                        },
                                    ))
                            })),
                    )
                    .child(
                        row()
                            .child(self.compose_button(
                                "Group",
                                "compose-group",
                                page.selected.len() > 1,
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.group(), cx),
                            ))
                            .child(self.compose_button(
                                "Ungroup",
                                "compose-ungroup",
                                selected.is_some_and(|o| o.group.is_some()),
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.ungroup(), cx),
                            )),
                    )
                    .child(
                        row()
                            .child(self.compose_button(
                                "Duplicate",
                                "compose-duplicate",
                                selected.is_some(),
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.duplicate(), cx),
                            ))
                            .child(self.compose_button(
                                "Remove",
                                "compose-remove",
                                selected.is_some(),
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.remove(), cx),
                            )),
                    ),
            ));
        if let Some(o) = selected {
            side = side.child(section(
                "Transform",
                column()
                    .child(muted(format!(
                        "{} selected · edits apply to selection",
                        page.selected.len()
                    )))
                    .child(self.compose_field(
                        "Centre X (mm)",
                        Property::X,
                        format!("{:.1}", o.center[0]),
                        cx,
                    ))
                    .child(self.compose_field(
                        "Centre Y (mm)",
                        Property::Y,
                        format!("{:.1}", o.center[1]),
                        cx,
                    ))
                    .child(self.compose_field(
                        "Width (mm)",
                        Property::Width,
                        format!("{:.1}", o.size[0]),
                        cx,
                    ))
                    .child(self.compose_field(
                        "Height (mm)",
                        Property::Height,
                        format!("{:.1}", o.size[1]),
                        cx,
                    ))
                    .child(self.compose_field(
                        "Rotation (°)",
                        Property::Rotation,
                        format!("{:.1}", o.rotation),
                        cx,
                    ))
                    .child(
                        row()
                            .child(self.compose_button(
                                "Flip ↔",
                                "compose-flip-x",
                                true,
                                cx,
                                |s, cx| {
                                    s.compose_edit_page(
                                        |p| p.edit_selected(|o| o.flip[0] = !o.flip[0]),
                                        cx,
                                    )
                                },
                            ))
                            .child(self.compose_button(
                                "Flip ↕",
                                "compose-flip-y",
                                true,
                                cx,
                                |s, cx| {
                                    s.compose_edit_page(
                                        |p| p.edit_selected(|o| o.flip[1] = !o.flip[1]),
                                        cx,
                                    )
                                },
                            )),
                    )
                    .child(
                        row()
                            .child(self.compose_button(
                                "Centre ↔",
                                "compose-center-x",
                                true,
                                cx,
                                |s, cx| {
                                    s.compose_edit_page(
                                        |p| {
                                            let x = p.size[0] / 2.;
                                            p.edit_selected(|o| o.center[0] = x);
                                        },
                                        cx,
                                    )
                                },
                            ))
                            .child(self.compose_button(
                                "Centre ↕",
                                "compose-center-y",
                                true,
                                cx,
                                |s, cx| {
                                    s.compose_edit_page(
                                        |p| {
                                            let y = p.size[1] / 2.;
                                            p.edit_selected(|o| o.center[1] = y);
                                        },
                                        cx,
                                    )
                                },
                            )),
                    )
                    .child(
                        row()
                            .child(self.compose_button(
                                "To front",
                                "compose-front",
                                true,
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.reorder(true), cx),
                            ))
                            .child(self.compose_button(
                                "To back",
                                "compose-back",
                                true,
                                cx,
                                |s, cx| s.compose_edit_page(|p| p.reorder(false), cx),
                            )),
                    ),
            ));
        }
        let fill = selected.map_or(self.compose.fill, |o| o.fill);
        let stroke = selected.map_or(self.compose.stroke, |o| o.stroke);
        side = side.child(section(
            "Colour",
            column()
                .child(self.choices(
                    "compose-colour-target",
                    &["Fill", "Stroke"],
                    self.compose.colour_target,
                    cx,
                    |s, i| s.compose.colour_target = i,
                ))
                .child(self.compose_field(
                    "Fill (#hex / none)",
                    Property::Fill,
                    fill.map_or("none".into(), |c| format!("#{c:06x}")),
                    cx,
                ))
                .child(self.compose_field(
                    "Stroke (#hex / none)",
                    Property::Stroke,
                    stroke.map_or("none".into(), |c| format!("#{c:06x}")),
                    cx,
                ))
                .child(self.compose_field(
                    "Weight (mm)",
                    Property::StrokeWidth,
                    format!(
                        "{:.2}",
                        selected.map_or(self.compose.stroke_width, |o| o.stroke_width)
                    ),
                    cx,
                ))
                .child(
                    row().gap(px(6.)).children(
                        [
                            0xffffff, 0x222222, 0x315ed8, 0xd44a54, 0xe9bc46, 0x49866c, 0xb49bc8,
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, c)| {
                            div()
                                .id(("compose-swatch", i))
                                .size(px(25.))
                                .border_1()
                                .border_color(rgb(LINE))
                                .bg(rgb(c))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.compose_apply_colour(Some(c), cx)
                                }))
                        }),
                    ),
                )
                .child(
                    self.compose_button("None", "compose-no-colour", true, cx, |s, cx| {
                        s.compose_apply_colour(None, cx)
                    }),
                ),
        ));
        side.child(section(
            "Output",
            column()
                .child(self.compose_button(
                    "Send to Print",
                    "compose-to-print",
                    self.can_send_composition_to_print(),
                    cx,
                    |s, cx| s.compose_send_to_print(cx),
                ))
                .child(self.compose_button(
                    "Save as SVG",
                    "compose-save",
                    self.can_export_composition(),
                    cx,
                    |s, cx| s.save_composition(cx),
                ))
                .child(muted("SVG preserves vectors and embeds imported images.")),
        ))
    }
    fn compose_button(
        &self,
        title: &'static str,
        id: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        action: fn(&mut Self, &mut Context<Self>),
    ) -> gpui::Stateful<Div> {
        button(title)
            .id(id)
            .debug_selector(move || id.into())
            .when(enabled, |b| {
                b.cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            })
            .when(!enabled, |b| b.opacity(0.4))
    }
    pub fn compose_stage(&self, cx: &mut Context<Self>) -> Div {
        let bounds = self.compose.viewport.clone();
        let page = self.compose.document.page.clone();
        let actual = self.compose.actual_size;
        let pending = self.compose.pending.clone();
        let tool = self.compose_tool;
        let preview = self.compose.preview.clone();
        // Preview and editor overlays use exactly the same physical-to-screen transform.
        let mut view = div()
            .id("compose-canvas")
            .debug_selector(|| "compose-canvas".into())
            .relative()
            .min_w_0()
            .min_h_0()
            .size_full();
        if actual {
            view = view
                .w(px(page.size[0] * 96. / 25.4 + 48.))
                .h(px(page.size[1] * 96. / 25.4 + 48.))
                .flex_shrink_0();
        }
        view = view
            .child(
                canvas(
                    move |rect, _, _| {
                        *bounds.borrow_mut() = rect;
                        rect
                    },
                    move |rect, _, window, cx| {
                        let Some(t) = ViewTransform::fit(
                            [rect.origin.x.into(), rect.origin.y.into()],
                            [
                                f32::from(rect.size.width) - 48.,
                                f32::from(rect.size.height) - 48.,
                            ],
                            page.size,
                            actual,
                        ) else {
                            return;
                        };
                        let t = ViewTransform {
                            origin: [t.origin[0] + 24., t.origin[1] + 24.],
                            ..t
                        };
                        let origin = t.screen([0., 0.]);
                        let size = page.size.map(|v| px(v * t.scale));
                        let page_bounds = Bounds::new(
                            point(px(origin[0]), px(origin[1])),
                            gpui::size(size[0], size[1]),
                        );
                        window.paint_quad(gpui::fill(page_bounds, rgb(WHITE)));
                        if let Some(image) = &preview {
                            let _ = window.paint_image(
                                page_bounds,
                                gpui::Corners::default(),
                                image.clone(),
                                0,
                                false,
                            );
                        }
                        window.paint_quad(gpui::outline(
                            page_bounds,
                            rgb(LINE),
                            Default::default(),
                        ));
                        let pos = |p: [f32; 2]| {
                            let p = t.screen(p);
                            point(px(p[0]), px(p[1]))
                        };
                        let line =
                            |points: Vec<[f32; 2]>, closed, color, window: &mut gpui::Window| {
                                if points.is_empty() {
                                    return;
                                }
                                let mut path = PathBuilder::stroke(px(1.));
                                path.move_to(pos(points[0]));
                                for &p in &points[1..] {
                                    path.line_to(pos(p));
                                }
                                if closed {
                                    path.close();
                                }
                                if let Ok(p) = path.build() {
                                    window.paint_path(p, rgb(color));
                                }
                            };
                        for o in page
                            .objects
                            .iter()
                            .filter(|o| page.selected.contains(&o.id))
                        {
                            line(
                                [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
                                    .map(|p| o.world(p))
                                    .to_vec(),
                                true,
                                0x315ed8,
                                window,
                            );
                        }
                        let handles: Vec<_> = if tool == 1 {
                            page.selection()
                                .and_then(|o| {
                                    o.vertices()
                                        .map(|points| points.iter().map(|&p| o.world(p)).collect())
                                })
                                .unwrap_or_default()
                        } else {
                            page.selection_frame()
                                .map(|o| {
                                    vec![
                                        o.world([1., 1.]),
                                        o.world([0.5, -8. / t.scale / o.size[1]]),
                                    ]
                                })
                                .unwrap_or_default()
                        };
                        for p in handles {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    pos(p) - point(px(4.), px(4.)),
                                    gpui::size(px(8.), px(8.)),
                                ),
                                rgb(0x315ed8),
                            ));
                        }
                        line(pending.clone(), false, 0x315ed8, window);
                        for &p in &pending {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    pos(p) - point(px(3.), px(3.)),
                                    gpui::size(px(6.), px(6.)),
                                ),
                                rgb(0x315ed8),
                            ));
                        }
                        let _ = cx;
                    },
                )
                .absolute()
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle);
                    this.start_compose_drag(event, cx);
                }),
            );
        column()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_0()
            .child(
                div()
                    .id("compose-viewport")
                    .flex_1()
                    .h(px(0.))
                    .min_h_0()
                    .min_w_0()
                    .overflow_scroll()
                    .child(view),
            )
            .child(self.compose_library(cx))
    }
    fn compose_transform(&self) -> Option<ViewTransform> {
        let rect = *self.compose.viewport.borrow();
        let mut t = ViewTransform::fit(
            [rect.origin.x.into(), rect.origin.y.into()],
            [
                f32::from(rect.size.width) - 48.,
                f32::from(rect.size.height) - 48.,
            ],
            self.compose.document.page.size,
            self.compose.actual_size,
        )?;
        t.origin[0] += 24.;
        t.origin[1] += 24.;
        Some(t)
    }
    fn start_compose_drag(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if !self.commit_compose_input(cx) {
            return;
        }
        let Some(t) = self.compose_transform() else {
            return;
        };
        let p = t.world([event.position.x.into(), event.position.y.into()]);
        let page = &self.compose.document.page;
        if self.compose_tool >= 2
            && (p[0] < 0. || p[1] < 0. || p[0] > page.size[0] || p[1] > page.size[1])
        {
            return;
        }
        if self.compose_tool == 2 {
            if self.compose.pending.len() >= 3
                && distance(p, self.compose.pending[0]) < 9. / t.scale
            {
                self.compose_finish_path(true, cx);
            } else {
                self.compose.pending.push(p);
                cx.notify();
            }
            return;
        }
        if self.compose_tool == 3 {
            let mut o = Object::new(Content::Text("Text".into()), p, [45., 15.]);
            o.fill = self.compose.fill;
            o.stroke = self.compose.stroke;
            self.compose_edit_page(
                |p| {
                    p.add(o);
                },
                cx,
            );
            self.compose_tool = 0;
            self.begin_compose_input(Property::Text, "Text".into(), cx);
            return;
        }
        if self.compose_tool == 4 {
            let shape = [
                Shape::Rectangle,
                Shape::Ellipse,
                Shape::Polygon,
                Shape::Star,
            ][self.compose.shape];
            let mut o = Object::new(Content::Shape(shape), p, [0.1, 0.1]);
            o.fill = self.compose.fill;
            o.stroke = self.compose.stroke;
            o.stroke_width = self.compose.stroke_width;
            self.compose.document.checkpoint();
            self.compose.document.page.add(o);
            self.compose.drag = Some(Drag {
                kind: DragKind::Create,
                start: p,
                originals: Vec::new(),
                checkpointed: true,
                frame: None,
            });
            self.compose_changed(cx);
            return;
        }
        let hit = page.hit(p, 4. / t.scale);
        if self.compose_tool == 5 {
            let fill = self.compose.fill;
            self.compose_edit_page(
                |page| {
                    if let Some(id) = hit {
                        page.select(Some(id), false);
                        page.edit_selected(|o| o.fill = fill);
                    }
                },
                cx,
            );
            return;
        }
        let mut kind = DragKind::Move;
        let mut handle = false;
        if let Some(o) = if self.compose_tool == 1 {
            page.selection().cloned()
        } else {
            page.selection_frame()
        } {
            if self.compose_tool == 1 {
                if let Some(points) = o.vertices()
                    && let Some(i) = points
                        .iter()
                        .position(|&q| distance(p, o.world(q)) < 9. / t.scale)
                {
                    kind = DragKind::Node(i);
                    handle = true;
                    self.compose.node = Some(i);
                }
            } else if distance(p, o.world([1., 1.])) < 9. / t.scale {
                kind = DragKind::Resize;
                handle = true;
            } else if distance(p, o.world([0.5, -8. / t.scale / o.size[1]])) < 9. / t.scale {
                kind = DragKind::Rotate;
                handle = true;
            }
        }
        if !handle {
            self.compose.node = None;
            let page = &mut self.compose.document.page;
            if event.modifiers.shift || hit.is_none_or(|id| !page.selected.contains(&id)) {
                page.select(hit, event.modifiers.shift);
            }
        }
        let originals = self
            .compose
            .document
            .page
            .objects
            .iter()
            .filter(|o| self.compose.document.page.selected.contains(&o.id))
            .cloned()
            .collect();
        self.compose.drag = Some(Drag {
            kind,
            start: p,
            originals,
            checkpointed: false,
            frame: self.compose.document.page.selection_frame(),
        });
        cx.notify();
    }
    pub fn move_compose_pointer(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.screen != Screen::Compose || event.pressed_button != Some(MouseButton::Left) {
            self.end_compose_drag();
            return;
        }
        let Some(t) = self.compose_transform() else {
            return;
        };
        let p = t.world([event.position.x.into(), event.position.y.into()]);
        let Some(drag) = &mut self.compose.drag else {
            return;
        };
        let delta: [f32; 2] = std::array::from_fn(|k| p[k] - drag.start[k]);
        if delta[0].abs() + delta[1].abs() < 0.001 {
            return;
        }
        if !drag.checkpointed {
            self.compose.document.checkpoint();
            drag.checkpointed = true;
        }
        if let DragKind::Create = drag.kind {
            self.compose.document.page.edit_selected(|o| {
                o.center = std::array::from_fn(|k| (p[k] + drag.start[k]) / 2.);
                o.size = std::array::from_fn(|k| delta[k].abs().max(0.1));
            });
        } else {
            for (index, original) in drag.originals.iter().enumerate() {
                if matches!(drag.kind, DragKind::Node(_)) && index != 0 {
                    continue;
                }
                if let Some(o) = self
                    .compose
                    .document
                    .page
                    .objects
                    .iter_mut()
                    .find(|o| o.id == original.id)
                {
                    *o = original.clone();
                    match drag.kind {
                        DragKind::Move => {
                            o.center = std::array::from_fn(|k| original.center[k] + delta[k])
                        }
                        DragKind::Resize => {
                            if drag.originals.len() > 1 {
                                let frame = drag.frame.as_ref().unwrap();
                                let factor = (distance(p, frame.center)
                                    / distance(drag.start, frame.center).max(0.01))
                                .clamp(0.01, 100.);
                                o.center = std::array::from_fn(|k| {
                                    frame.center[k]
                                        + (original.center[k] - frame.center[k]) * factor
                                });
                                o.size = original.size.map(|v| (v * factor).clamp(0.1, 4000.));
                            } else {
                                let q = original.local(p);
                                o.size = std::array::from_fn(|k| {
                                    (original.size[k] * (2. * (q[k] - 0.5)).abs()).clamp(0.1, 4000.)
                                });
                            }
                        }
                        DragKind::Rotate => {
                            let center = drag.frame.as_ref().unwrap().center;
                            let angle = |q: [f32; 2]| (q[1] - center[1]).atan2(q[0] - center[0]);
                            let rotation = angle(p) - angle(drag.start);
                            let (sin, cos) = rotation.sin_cos();
                            let dx = original.center[0] - center[0];
                            let dy = original.center[1] - center[1];
                            o.center = [
                                center[0] + cos * dx - sin * dy,
                                center[1] + sin * dx + cos * dy,
                            ];
                            o.rotation =
                                (original.rotation + rotation.to_degrees()).rem_euclid(360.);
                        }
                        DragKind::Node(i) => {
                            let q = original.local(p);
                            if !matches!(o.content, Content::Path { .. })
                                && let Some(points) = o.vertices()
                            {
                                o.content = Content::Path {
                                    points,
                                    closed: true,
                                };
                            }
                            if let Content::Path { points, .. } = &mut o.content
                                && let Some(node) = points.get_mut(i)
                            {
                                *node = q;
                            }
                        }
                        DragKind::Create => {}
                    }
                }
            }
        }
        self.compose_changed(cx);
    }
    pub fn end_compose_drag(&mut self) {
        if let Some(drag) = self.compose.drag.take()
            && matches!(drag.kind, DragKind::Node(_))
            && drag.checkpointed
        {
            self.compose
                .document
                .page
                .edit_selected(|o| o.normalize_path());
        }
    }
    fn compose_library(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap(px(8.))
            .flex_shrink_0()
            .px(px(20.))
            .py(px(12.))
            .bg(rgb(PANEL))
            .border_t_1()
            .border_color(rgb(LINE))
            .child(
                row()
                    .child(heading("LIBRARY").text_xs())
                    .child(self.choices(
                        "compose-library",
                        &["All", "Convert", "Print", "Files"],
                        self.compose.filter,
                        cx,
                        |s, i| s.compose.filter = i,
                    ))
                    .child(div().flex_1())
                    .child(self.compose_button(
                        "+ Add file",
                        "compose-add-file",
                        true,
                        cx,
                        |s, cx| s.open_compose_file(cx),
                    )),
            )
            .child(
                div().id("compose-library-items").overflow_x_scroll().child(
                    row()
                        .min_h(px(60.))
                        .when(self.compose.library.is_empty(), |r| {
                            r.child(muted(
                                "Add images, or use Add to Compose in Convert / Print",
                            ))
                        })
                        .children(
                            self.compose
                                .library
                                .iter()
                                .enumerate()
                                .filter(|(_, a)| match self.compose.filter {
                                    1 => a.art.origin == Origin::Convert,
                                    2 => a.art.origin == Origin::Print,
                                    3 => a.art.origin == Origin::File,
                                    _ => true,
                                })
                                .map(|(i, a)| {
                                    column()
                                        .id(("compose-library-item", i))
                                        .w(px(110.))
                                        .flex_shrink_0()
                                        .p(px(4.))
                                        .bg(rgb(BUTTON))
                                        .cursor_pointer()
                                        .child(
                                            img(a.thumbnail.clone())
                                                .w_full()
                                                .h(px(42.))
                                                .object_fit(ObjectFit::Contain),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(a.art.name.clone()),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let a = this.compose.library[i].art.clone();
                                            this.place_compose_artwork(a, cx);
                                        }))
                                }),
                        ),
                ),
            )
    }
    fn place_compose_artwork(&mut self, art: Arc<Artwork>, cx: &mut Context<Self>) {
        let size = self.compose.document.page.size;
        let width = (size[0] * 0.6).min(size[1] * 0.6 * art.aspect);
        let height = width / art.aspect;
        self.compose_edit_page(
            |p| {
                p.add(Object::new(
                    Content::Artwork(art),
                    size.map(|v| v / 2.),
                    [width, height],
                ));
            },
            cx,
        );
    }
    fn finish_compose_import(
        &mut self,
        result: Result<(Artwork, Arc<RenderImage>), String>,
        cx: &mut Context<Self>,
    ) {
        self.compose.imports = self.compose.imports.saturating_sub(1);
        match result {
            Ok((art, thumbnail)) => {
                self.compose.status = format!(
                    "Added {} · click a library item to place another copy",
                    art.name
                );
                let art = Arc::new(art);
                self.compose.library.push(LibraryItem {
                    art: art.clone(),
                    thumbnail,
                });
                self.place_compose_artwork(art, cx);
            }
            Err(e) => {
                self.compose.status = format!("Could not add artwork: {e}");
                cx.notify();
            }
        }
    }
    pub fn open_compose_file(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add images to the page".into()),
        });
        cx.spawn(async move |this, cx| match prompt.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    for path in paths {
                        this.load_compose_path(path, cx);
                    }
                });
            }
            Ok(Err(e)) => {
                let _ = this.update(cx, |this, cx| {
                    this.compose.status = format!("Could not open dialog: {e}");
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    pub fn load_compose_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(parent) = path.parent() {
            self.compose.directory = parent.to_owned();
        }
        self.compose.imports += 1;
        self.compose.status = "Loading artwork…".into();
        let renderer = cx.svg_renderer();
        let task = cx.background_executor().spawn(async move {
            let mut reader = image::ImageReader::open(&path)
                .map_err(|e| e.to_string())?
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(512 * 1024 * 1024);
            reader.limits(limits);
            let rgba = reader
                .decode()
                .map_err(|e| e.to_string())?
                .thumbnail(4096, 4096)
                .to_rgba8();
            let aspect = rgba.width() as f32 / rgba.height() as f32;
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(rgba)
                .write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            let art = Artwork {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                origin: Origin::File,
                aspect,
                uri: composition::data_uri("image/png", png.get_ref()),
            };
            let mut page = composition::Page::default();
            page.size = [100. * aspect, 100.];
            page.add(Object::new(
                Content::Artwork(Arc::new(art.clone())),
                page.size.map(|v| v / 2.),
                page.size,
            ));
            let thumb = raster(&page.svg(), page.size, 100., |image| {
                image.to_image_data(renderer).map_err(|e| e.to_string())
            })?;
            Ok((art, thumb))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.finish_compose_import(result, cx));
        })
        .detach();
        cx.notify();
    }
    pub fn send_to_compose(&mut self, cx: &mut Context<Self>) {
        if !self.can_export_svg() || !matches!(self.screen, Screen::Convert | Screen::Print) {
            return;
        }
        let Some(source) = self.editor.source.clone() else {
            return;
        };
        let mut params = self.editor.params;
        params.operation = if self.screen == Screen::Print {
            Operation::Print
        } else {
            Operation::Convert
        };
        params.max_side = 2048;
        params.upscale = 1.;
        let origin = if self.screen == Screen::Print {
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
        self.screen = Screen::Compose;
        self.compose.imports += 1;
        self.compose.status = "Preparing vector snapshot…".into();
        self.render_compose(cx);
        let renderer = cx.svg_renderer();
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
            engine.write_svg(params, true);
            let size = if params.operation == Operation::Print {
                [
                    params
                        .page_width
                        .max(engine.info.art_mm[0] + 2. * params.margin),
                    params
                        .page_height
                        .max(engine.info.art_mm[1] + 2. * params.margin),
                ]
            } else {
                [engine.info.width as f32, engine.info.height as f32]
            };
            let thumb = raster(&engine.svg, size, 100., |image| {
                image.to_image_data(renderer).map_err(|e| e.to_string())
            })?;
            Ok((
                Artwork {
                    name,
                    origin,
                    aspect: size[0] / size[1],
                    uri: composition::data_uri("image/svg+xml", engine.svg.as_bytes()),
                },
                thumb,
            ))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.finish_compose_import(result, cx));
        })
        .detach();
        cx.notify();
    }
    pub fn can_export_composition(&self) -> bool {
        !self.compose.exporting
            && self.compose.imports == 0
            && !self.compose.document.page.objects.is_empty()
    }
    pub fn save_composition(&mut self, cx: &mut Context<Self>) {
        if !self.commit_compose_input(cx) || !self.can_export_composition() {
            return;
        }
        let svg = self.compose.document.page.svg();
        let prompt = cx.prompt_for_new_path(&self.compose.directory, Some("composition.svg"));
        self.compose.exporting = true;
        cx.spawn(async move |this, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(path))) => path,
                result => {
                    let _ = this.update(cx, |this, cx| {
                        this.compose.exporting = false;
                        this.compose.status = match result {
                            Ok(Err(e)) => format!("Export dialog failed: {e}"),
                            _ => "Export cancelled".into(),
                        };
                        cx.notify();
                    });
                    return;
                }
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::fs::write(&path, svg)
                        .map(|()| path)
                        .map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.compose.exporting = false;
                this.compose.status = match result {
                    Ok(path) => {
                        this.compose.directory = path.parent().unwrap_or(Path::new(".")).to_owned();
                        format!("Saved {}", path.display())
                    }
                    Err(e) => format!("Export failed: {e}"),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn can_send_composition_to_print(&self) -> bool {
        self.can_export_composition()
            && !self.editor.loading
            && !self.editor.busy
            && !self.editor.exporting
    }
    pub fn compose_send_to_print(&mut self, cx: &mut Context<Self>) {
        if !self.commit_compose_input(cx)
            || !self.can_export_composition()
            || self.editor.loading
            || self.editor.busy
            || self.editor.exporting
        {
            return;
        }
        let page = self.compose.document.page.clone();
        let size = page.size;
        let renderer = cx.svg_renderer();
        self.editor.load_revision += 1;
        let revision = self.editor.load_revision;
        self.editor.loading = true;
        self.compose.status = "Preparing page for Print…".into();
        let task = cx.background_executor().spawn(async move {
            raster(&page.svg(), page.size, 4096., |image| {
                image.to_image_data(renderer).map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if revision != this.editor.load_revision {
                    return;
                }
                this.editor.loading = false;
                match result {
                    Ok(image) => {
                        if let Some(old) = this.editor.source.replace(image) {
                            this.editor.retired.push(old);
                        }
                        if let Some(old) = this.editor.preview.take() {
                            this.editor.retired.push(old);
                        }
                        this.editor.path = Some(PathBuf::from("composition.png"));
                        this.editor.histogram = [0; 256];
                        if let Some(source) = &this.editor.source
                            && let Some(bytes) = source.as_bytes(0)
                        {
                            let size = source.size(0);
                            if let Ok(v) = ImageView::new(
                                bytes,
                                size.width.0 as usize,
                                size.height.0 as usize,
                                size.width.0 as usize * 4,
                            ) {
                                for i in 0..v.len() {
                                    this.editor.histogram[(effects::color::luma(v.rgb(i)) * 255.)
                                        .round()
                                        as usize] += 1;
                                }
                            }
                        }
                        this.editor.params.page_width = size[0];
                        this.editor.params.page_height = size[1];
                        this.editor.params.margin = 0.;
                        this.editor.params.art_width = size[0];
                        this.screen = Screen::Print;
                        this.plate = 0;
                        this.request_effects(cx);
                        this.compose.status = "Sent page to Print".into();
                    }
                    Err(e) => this.compose.status = format!("Could not send to Print: {e}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
