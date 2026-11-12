use bitman::{
    effects::{Engine, ImageView, Method, Operation, Params},
    simulation::{Artwork, EXPORT_WIDTH, Origin, PREVIEW_WIDTH, Simulation, ViewTransform},
};
use std::sync::Arc;

fn artwork(bgra: [u8; 4]) -> Arc<Artwork> {
    Arc::new(Artwork::from_bgra("test".into(), Origin::File, 4, 2, &bgra.repeat(8)).unwrap())
}
fn pixel(image: &tiny_skia::Pixmap, point: [f32; 2]) -> [u8; 4] {
    let t = ViewTransform::fit([0., 0.], [image.width() as f32, image.height() as f32]).unwrap();
    let p = t.screen(point);
    let i = (p[1].round() as usize * image.width() as usize + p[0].round() as usize) * 4;
    image.data()[i..i + 4].try_into().unwrap()
}

#[test]
fn viewport_mapping_is_invertible_with_letterboxing_and_scroll_offsets() {
    for size in [[800., 800.], [1400., 400.], [1000., 700.]] {
        let t = ViewTransform::fit([-205., 72.], size).unwrap();
        for p in [[0., 0.], [-60., 75.], [27.5, 73.]] {
            let q = t.world(t.screen(p));
            assert!((p[0] - q[0]).abs() < 0.0001 && (p[1] - q[1]).abs() < 0.0001);
        }
    }
    assert!(ViewTransform::fit([0., 0.], [0., 500.]).is_none());
}

#[test]
fn sides_are_independent_and_duplicates_share_only_artwork() {
    let mut s = Simulation::default();
    s.add(artwork([0, 0, 255, 255]));
    s.duplicate();
    s.selection_mut().unwrap().x = 17.;
    assert_eq!(s.current()[0].x, 0.);
    assert!(Arc::ptr_eq(
        &s.current()[0].artwork,
        &s.current()[1].artwork
    ));
    s.set_side(true);
    assert!(s.selection().is_none() && s.current().is_empty());
    s.add(artwork([255, 0, 0, 255]));
    s.remove();
    assert!(s.current().is_empty());
    s.set_side(false);
    assert_eq!(s.current().len(), 2);
}

#[test]
fn rotated_hit_testing_uses_design_coordinates_and_topmost_order() {
    let mut s = Simulation::default();
    s.add(artwork([0, 0, 255, 255]));
    s.selection_mut().unwrap().rotation = 90.;
    let d = s.selection().unwrap();
    let centre = d.centre(s.collar());
    assert_eq!(s.hit(centre), Some(0));
    assert!(d.contains([centre[0], centre[1] + 10.], s.collar()));
    assert!(!d.contains([centre[0] + 10., centre[1]], s.collar()));
    s.duplicate();
    assert_eq!(s.hit(centre), Some(1));
}

#[test]
fn resizing_preserves_ratio_and_valid_bounds() {
    let mut s = Simulation::default();
    s.add(artwork([0, 0, 0, 255]));
    let d = s.selection_mut().unwrap();
    d.set_width(40.);
    assert_eq!((d.width, d.height), (40., 20.));
    d.set_height(100.);
    assert_eq!((d.width, d.height), (100., 50.));
    d.set_width(0.);
    assert_eq!((d.width, d.height), (1., 0.5));
    d.keep_proportions = false;
    d.set_width(35.);
    assert_eq!((d.width, d.height), (35., 0.5));
}

#[test]
fn transparent_artwork_and_white_ink_composite_correctly_on_dark_shirts() {
    let mut s = Simulation {
        color: [20, 40, 60],
        measurements: false,
        ..Default::default()
    };
    s.add(artwork([0, 0, 255, 128]));
    let centre = s.selection().unwrap().centre(s.collar());
    let image = s.render(PREVIEW_WIDTH, false, false).unwrap();
    let p = pixel(&image, centre);
    assert!((i32::from(p[0]) - 138).abs() <= 1, "{p:?}");
    assert_eq!(&p[1..], &[20, 30, 255]);
    s.remove();
    s.add(artwork([255, 255, 255, 255]));
    assert_eq!(
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), centre),
        [255; 4]
    );
    s.remove();
    s.add(artwork([255, 0, 255, 0]));
    assert_eq!(
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), centre),
        [20, 40, 60, 255]
    );
}

#[test]
fn clipping_follows_garment_outline_and_can_be_disabled() {
    let mut s = Simulation::default();
    s.add(artwork([0, 0, 255, 255]));
    let d = s.selection_mut().unwrap();
    d.x = 25.;
    d.top = 40.;
    d.set_width(40.);
    let clipped = s.render(PREVIEW_WIDTH, false, false).unwrap();
    assert_eq!(pixel(&clipped, [20., 55.]), [255, 0, 0, 255]);
    assert_eq!(pixel(&clipped, [40., 55.]), [242, 243, 245, 255]);
    s.crop = false;
    assert_eq!(
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), [40., 55.]),
        [255, 0, 0, 255]
    );
}

#[test]
fn preview_and_export_have_identical_placement_without_editor_handles() {
    let mut s = Simulation {
        color: [50, 90, 140],
        ..Default::default()
    };
    s.add(artwork([0, 0, 255, 255]));
    let d = s.selection_mut().unwrap();
    d.rotation = 37.;
    d.x = 4.;
    d.top = 16.;
    let preview = s.render(PREVIEW_WIDTH, false, false).unwrap();
    let export = s.render(EXPORT_WIDTH, false, false).unwrap();
    for p in [[0., 35.], [4., 30.], [-20., 65.], [60., 70.], [30., 65.]] {
        assert_eq!(pixel(&preview, p), pixel(&export, p), "at {p:?}");
    }
    assert_ne!(
        preview.data(),
        s.render(PREVIEW_WIDTH, false, true).unwrap().data()
    );
    s.selected = None;
    assert_eq!(
        preview.data(),
        s.render(PREVIEW_WIDTH, false, true).unwrap().data()
    );
}

#[test]
fn sleeve_size_back_and_measurement_options_change_geometry() {
    let mut s = Simulation::default();
    let base = s.render(PREVIEW_WIDTH, false, false).unwrap();
    s.long_sleeve = true;
    assert_ne!(
        pixel(&base, [49., 42.]),
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), [49., 42.])
    );
    s.long_sleeve = false;
    s.size = 0;
    assert_ne!(
        pixel(&base, [25., 60.]),
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), [25., 60.])
    );
    s.size = 3;
    s.set_side(true);
    assert_ne!(
        pixel(&base, [0., 4.]),
        pixel(&s.render(PREVIEW_WIDTH, false, false).unwrap(), [0., 4.])
    );
    s.set_side(false);
    assert_ne!(
        base.data(),
        s.render(PREVIEW_WIDTH, true, false).unwrap().data()
    );
}

#[test]
fn jpeg_is_decodable_opaque_and_uses_export_measurement_option() {
    let mut s = Simulation::default();
    s.add(artwork([0, 0, 255, 255]));
    let mut bytes = Vec::new();
    s.write_jpeg(&mut bytes, 1000).unwrap();
    assert_eq!(&bytes[..2], &[0xff, 0xd8]);
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (1000, 700));
    let a = bytes;
    s.selected = None;
    s.measurements = false;
    let mut b = Vec::new();
    s.write_jpeg(&mut b, 1000).unwrap();
    assert_eq!(
        a, b,
        "selection and preview measurements must not leak into export"
    );
    s.export_measurements = true;
    let mut c = Vec::new();
    s.write_jpeg(&mut c, 1000).unwrap();
    assert_ne!(b, c);
}

#[test]
fn print_snapshot_removes_only_unprinted_background() {
    // A dark print with white ink must retain white ink instead of colour-keying it.
    let p = Params {
        operation: Operation::Print,
        method: Method::One,
        dark: true,
        dark_bg: [20, 30, 40],
        ink: [255; 3],
        max_side: 64,
        ..Params::default()
    };
    let mut source = vec![[255, 255, 255, 255]; 64 * 64];
    for y in 16..48 {
        for x in 16..48 {
            source[y * 64 + x] = [0, 0, 0, 255];
        }
    }
    let mut e = Engine::default();
    e.render(
        ImageView::new(source.as_flattened(), 64, 64, 256).unwrap(),
        p,
    )
    .unwrap();
    e.composite_artwork(p);
    let n = e.info.width * e.info.height;
    let mut ink = 0;
    let mut clear = 0;
    for (i, pixel) in e.pixels.iter().enumerate() {
        let covered = (0..e.info.plates).any(|k| e.masks[k * n + i] != 0);
        if covered {
            assert_eq!(pixel[3], 255);
            ink += 1;
        } else {
            assert_eq!(*pixel, [0; 4]);
            clear += 1;
        }
    }
    assert!(ink > 0 && clear > 0);
}

#[test]
fn invalid_buffers_and_render_sizes_fail_without_panicking() {
    assert!(Artwork::from_bgra("bad".into(), Origin::File, 2, 2, &[0; 4]).is_err());
    assert!(Artwork::from_bgra("bad".into(), Origin::File, 0, 2, &[]).is_err());
    assert!(Simulation::default().render(0, false, false).is_err());
    assert!(
        Simulation::default()
            .render(u32::MAX, false, false)
            .is_err()
    );
}

#[test]
fn missing_jpeg_extension_never_overwrites_an_unconfirmed_file() {
    let directory = tempfile::tempdir().unwrap();
    let name = directory.path().join("mockup");
    let destination = name.with_extension("jpg");
    std::fs::write(&destination, b"keep this file").unwrap();
    let s = Simulation::default();
    assert!(s.save_jpeg(&name).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"keep this file");
    assert!(s.save_jpeg(&name.with_extension("png")).is_err());
    assert!(!name.with_extension("png").exists());
    let new_name = directory.path().join("new-mockup");
    assert_eq!(
        s.save_jpeg(&new_name).unwrap(),
        new_name.with_extension("jpg")
    );
    let decoded = image::open(new_name.with_extension("jpg")).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (3000, 2100));
}

#[test]
fn removing_selected_placement_leaves_other_layers_sides_and_library_artwork() {
    let art = artwork([0, 0, 255, 255]);
    let mut s = Simulation::default();
    s.add(art.clone());
    s.duplicate();
    s.current_mut()[1].x = 12.;
    s.set_side(true);
    s.add(art.clone());
    s.set_side(false);
    s.selected = Some(0);
    s.remove();
    assert_eq!(s.current().len(), 1);
    assert_eq!(s.current()[0].x, 12.);
    assert!(s.selected.is_none());
    s.remove(); // No selection must not delete another layer.
    assert_eq!(s.current().len(), 1);
    s.set_side(true);
    assert_eq!(s.current().len(), 1);
    s.add(art); // The same library item remains reusable.
    assert_eq!(s.current().len(), 2);
}
