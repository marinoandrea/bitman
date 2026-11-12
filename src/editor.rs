//! GPUI owns decoding and GPU uploads; the numerical engine owns reusable buffers.
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc, time::Instant};

use crate::app::{Screen, Workspace};
use bitman::effects::{
    self, Dither, Engine, ImageView, Info, Method, Mode, Operation, Params, Render, Shape,
};
use gpui::{
    Asset, Bounds, Context, Image, ImageAssetLoader, ImageFormat, PathPromptOptions, Pixels,
    RenderImage, Resource, Window,
};

pub struct Editor {
    pub params: Params,
    pub source: Option<Arc<RenderImage>>,
    pub preview: Option<Arc<RenderImage>>,
    pub retired: Vec<Arc<RenderImage>>,
    pub path: Option<PathBuf>,
    pub engine: Option<Engine>,
    pub info: Info,
    pub histogram: [usize; 256],
    pub busy: bool,
    pub loading: bool,
    pub exporting: bool,
    pub status: String,
    pub milliseconds: f64,
    revision: u64,
    load_revision: u64,
    last: Option<(Params, usize)>,
    pub bounds: Rc<RefCell<[Bounds<Pixels>; 64]>>,
    pub active_slider: Option<usize>,
    pub picking: bool,
    pub picking_garment: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            params: Params::default(),
            source: None,
            preview: None,
            retired: Vec::new(),
            path: None,
            engine: Some(Engine::default()),
            info: Info::default(),
            histogram: [0; 256],
            busy: false,
            loading: false,
            exporting: false,
            status: "Open an image to begin".into(),
            milliseconds: 0.0,
            revision: 0,
            load_revision: 0,
            last: None,
            bounds: Rc::new(RefCell::new([Bounds::default(); 64])),
            active_slider: None,
            picking: false,
            picking_garment: false,
        }
    }
}

impl Workspace {
    pub fn open_image(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::Simulate {
            self.open_simulation_file(cx);
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open image".into()),
        });
        cx.spawn(async move |this, cx| match prompt.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| this.load_path(path, cx));
                }
            }
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.editor.status = error.to_string();
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }

    pub fn load_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.editor.load_revision += 1;
        let revision = self.editor.load_revision;
        self.editor.loading = true;
        self.editor.status = "Decoding image…".into();
        let future = ImageAssetLoader::load(Resource::Path(path.clone().into()), cx);
        let task = cx.background_executor().spawn(async move {
            future.await.map(|image| {
                let mut histogram = [0usize; 256];
                let size = image.size(0);
                if let Some(bytes) = image.as_bytes(0)
                    && let Ok(view) = ImageView::new(
                        bytes,
                        size.width.0 as usize,
                        size.height.0 as usize,
                        size.width.0 as usize * 4,
                    )
                {
                    for i in 0..view.len() {
                        histogram[(effects::color::luma(view.rgb(i)) * 255.0).round() as usize] +=
                            1;
                    }
                }
                (image, histogram)
            })
        });
        cx.spawn(async move |this, cx| {
            let image = task.await;
            let _ = this.update(cx, |this, cx| {
                if revision != this.editor.load_revision {
                    return;
                }
                this.editor.loading = false;
                match image {
                    Ok((image, histogram)) => {
                        this.editor.histogram = histogram;
                        if let Some(old) = this.editor.source.replace(image) {
                            this.editor.retired.push(old);
                        }
                        if let Some(old) = this.editor.preview.take() {
                            this.editor.retired.push(old);
                        }
                        this.editor.path = Some(path);
                        this.request_effects(cx);
                    }
                    Err(error) => {
                        this.editor.status = format!("Could not open image: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn auto_levels(&mut self, cx: &mut Context<Self>) {
        let (black, white) = effects::filters::auto_levels(&self.editor.histogram);
        self.editor.params.black = black;
        self.editor.params.white = white;
        self.editor.params.gamma = 1.0;
        self.request_effects(cx);
    }

    pub fn choice_changed(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if matches!(id, "shirt-size" | "sleeve" | "shirt-side") {
            self.end_simulation_drag();
            self.simulation_changed(cx);
            return;
        }
        let p = &mut self.editor.params;
        match id {
            "convert-mode" => p.mode = [Mode::Auto, Mode::Lineart, Mode::Color][self.convert_mode],
            "upscale-style" => p.pixel = self.upscale_style == 1,
            "print-method" => {
                p.method = [Method::One, Method::Cmyk, Method::Spot][self.print_method];
                self.plate = 0;
            }
            "print-kind" => p.photo = self.print_kind == 1,
            "print-render" => p.render = [Render::Screen, Render::Dither][self.print_render],
            "print-shape" => {
                p.shape = [
                    Shape::Round,
                    Shape::Diamond,
                    Shape::Ellipse,
                    Shape::Line,
                    Shape::Square,
                    Shape::Cross,
                    Shape::Custom,
                ][self.print_shape]
            }
            "dither" => {
                p.dither = [
                    Dither::Floyd,
                    Dither::Atkinson,
                    Dither::Stucki,
                    Dither::Bayer,
                    Dither::Blue,
                    Dither::Serpentine,
                    Dither::Random,
                ][self.dither]
            }
            "plates" | "quantizer" | "pattern" => {}
            _ => return,
        }
        self.request_effects(cx);
    }

    pub fn request_effects(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Convert | Screen::Print) {
            return;
        }
        self.editor.params.operation = if self.screen == Screen::Print {
            Operation::Print
        } else {
            Operation::Convert
        };
        self.editor.revision += 1;
        if self.editor.busy {
            cx.notify();
            return;
        }
        let Some(source) = self.editor.source.clone() else {
            cx.notify();
            return;
        };
        let Some(mut engine) = self.editor.engine.take() else {
            return;
        };
        let params = self.editor.params;
        let source_id = source.id.0;
        let reuse = self.editor.last == Some((params, source_id));
        let revision = self.editor.revision;
        let screen = self.screen;
        let selected_plate = self.plate;
        let plate = if self.screen == Screen::Print && self.plate > 0 {
            Some(self.plate - 1)
        } else {
            None
        };
        let renderer = cx.svg_renderer();
        self.editor.busy = true;
        self.editor.status = "Processing…".into();
        let task = cx.background_executor().spawn(async move {
            let start = Instant::now();
            let result = (|| -> Result<Arc<RenderImage>, String> {
                if !reuse {
                    let size = source.size(0);
                    let bytes = source.as_bytes(0).ok_or("No pixels in image")?;
                    let view = ImageView::new(
                        bytes,
                        size.width.0 as usize,
                        size.height.0 as usize,
                        size.width.0 as usize * 4,
                    )
                    .map_err(|e| e.to_string())?;
                    engine.render(view, params).map_err(|e| e.to_string())?;
                }
                if params.operation == Operation::Convert {
                    // Keep the raster preview bounded even when export declares original dimensions.
                    engine.write_svg(
                        Params {
                            upscale: 1.0,
                            ..params
                        },
                        true,
                    );
                    let image = Image::from_bytes(
                        ImageFormat::Svg,
                        std::mem::take(&mut engine.svg).into_bytes(),
                    );
                    // GPUI 0.2.2's Image::to_image_data SVG branch omits RGBA/BGRA conversion
                    // and unpremultiplication. Correct that at this codec boundary, without a new crate.
                    let raster = image
                        .to_image_data(renderer.clone())
                        .map_err(|e| e.to_string());
                    engine.svg = String::from_utf8(image.bytes).expect("SVG stays UTF-8");
                    let raster = raster?;
                    let bytes = raster.as_bytes(0).ok_or("No rasterized SVG pixels")?;
                    if bytes.len() != engine.pixels.len() * 4 {
                        return Err("Unexpected SVG preview dimensions".into());
                    }
                    engine
                        .execution
                        .chunks(&mut engine.pixels, 4, |offset, part| {
                            for (i, pixel) in part.iter_mut().enumerate() {
                                let start = (offset + i) * 4;
                                *pixel = effects::color::premultiplied_rgba_to_bgra(
                                    bytes[start..start + 4].try_into().unwrap(),
                                );
                            }
                        });
                } else {
                    let plate = plate.map(|v| v.min(engine.info.plates.saturating_sub(1)));
                    engine.composite(plate, params);
                }
                engine.encode_bmp();
                let image = Image::from_bytes(ImageFormat::Bmp, std::mem::take(&mut engine.bmp));
                let result = image.to_image_data(renderer).map_err(|e| e.to_string());
                engine.bmp = image.bytes;
                result
            })();
            (engine, result, start.elapsed().as_secs_f64() * 1000.0)
        });
        cx.spawn(async move |this, cx| {
            let (engine, result, ms) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.editor.busy = false;
                this.editor.last = if result.is_ok() {
                    Some((params, source_id))
                } else {
                    None
                };
                // Show completed frames while controls keep changing; waiting for an
                // exact revision would starve the preview throughout a slider drag.
                // Frames from another image, workspace, or plate must still be ignored.
                if this.editor.source.as_ref().map(|source| source.id.0) == Some(source_id)
                    && this.screen == screen
                    && (screen != Screen::Print || this.plate == selected_plate)
                {
                    this.editor.info = engine.info;
                    if revision == this.editor.revision {
                        this.plate = this.plate.min(engine.info.plates);
                    }
                    this.editor.milliseconds = ms;
                    match result {
                        Ok(image) => {
                            if let Some(old) = this.editor.preview.replace(image) {
                                this.editor.retired.push(old);
                            }
                            this.editor.status = format!(
                                "{} × {} · {:.1} ms · {}",
                                engine.info.width,
                                engine.info.height,
                                ms,
                                effects::simd::backend()
                            );
                        }
                        Err(error) if revision == this.editor.revision => {
                            this.editor.status = error;
                        }
                        Err(_) => {}
                    }
                }
                this.editor.engine = Some(engine);
                if revision != this.editor.revision {
                    this.request_effects(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn can_export_svg(&self) -> bool {
        self.editor.source.is_some()
            && !self.editor.loading
            && !self.editor.busy
            && !self.editor.exporting
            && matches!(self.screen, Screen::Convert | Screen::Print)
    }

    pub fn save_svg(&mut self, cx: &mut Context<Self>) {
        if !self.can_export_svg() {
            return;
        }
        let path = self.editor.path.clone().unwrap_or_default();
        let name = format!(
            "{}-{}.svg",
            path.file_stem().unwrap_or_default().to_string_lossy(),
            if self.screen == Screen::Print {
                "plates"
            } else {
                "vector"
            }
        );
        let prompt = cx.prompt_for_new_path(
            path.parent().unwrap_or(std::path::Path::new(".")),
            Some(&name),
        );
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = prompt.await {
                let _ = this.update(cx, |this, cx| this.export_to(path, cx));
            }
        })
        .detach();
    }

    pub fn export_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.editor.busy || self.editor.exporting {
            self.editor.status = "Wait for the preview before exporting".into();
            cx.notify();
            return;
        }
        let Some(source) = self.editor.source.clone() else {
            return;
        };
        let Some(mut engine) = self.editor.engine.take() else {
            return;
        };
        let mut params = self.editor.params;
        params.max_side = effects::MAX_SIDE;
        self.editor.busy = true;
        self.editor.exporting = true;
        self.editor.status = "Exporting at up to 4096 pixels…".into();
        let task = cx.background_executor().spawn(async move {
            let result = (|| -> Result<(), String> {
                let size = source.size(0);
                let bytes = source.as_bytes(0).ok_or("No pixels in image")?;
                let view = ImageView::new(
                    bytes,
                    size.width.0 as usize,
                    size.height.0 as usize,
                    size.width.0 as usize * 4,
                )
                .map_err(|e| e.to_string())?;
                engine.render(view, params).map_err(|e| e.to_string())?;
                engine.write_svg(params, params.operation == Operation::Convert);
                std::fs::write(&path, &engine.svg).map_err(|e| e.to_string())
            })();
            (engine, result, path)
        });
        cx.spawn(async move |this, cx| {
            let (engine, result, path) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.editor.engine = Some(engine);
                this.editor.busy = false;
                this.editor.exporting = false;
                this.editor.last = None;
                let message = match result {
                    Ok(()) => format!("Saved {}", path.display()),
                    Err(e) => format!("Export failed: {e}"),
                };
                this.request_effects(cx);
                this.editor.status = message;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn release_retired_images(&mut self, window: &mut Window) {
        for image in self.editor.retired.drain(..) {
            let _ = window.drop_image(image);
        }
    }
}
