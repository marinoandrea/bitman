use bitman::composition::{Content, Document, Object, Page, Shape, ViewTransform, data_uri};
fn object(center: [f32; 2]) -> Object {
    Object::new(Content::Shape(Shape::Rectangle), center, [40., 20.])
}
fn close(a: [f32; 2], b: [f32; 2]) {
    for k in 0..2 {
        assert!((a[k] - b[k]).abs() < 0.001, "{a:?} != {b:?}");
    }
}

#[test]
fn transforms_and_hits_follow_rotated_flipped_geometry() {
    let mut o = object([100., 80.]);
    for rotation in [0., 31., 90., 200.] {
        for flip in [[false, false], [true, false], [false, true], [true, true]] {
            o.rotation = rotation;
            o.flip = flip;
            for p in [[0., 0.], [0.3, 0.8], [1., 1.], [-2., 3.]] {
                close(o.local(o.world(p)), p);
            }
            assert!(o.hit(o.world([0.3, 0.8]), 0.));
            assert!(!o.hit(o.world([1.5, 0.5]), 0.));
        }
    }
    o.content = Content::Shape(Shape::Ellipse);
    assert!(!o.hit(o.world([0.05, 0.05]), 0.));
    assert!(o.hit(o.world([0.5, 0.5]), 0.));
    o.content = Content::Path {
        points: vec![[0., 0.], [1., 1.]],
        closed: false,
    };
    assert!(o.hit(o.world([0.4, 0.4]), 0.1));
    assert!(!o.hit(o.world([0.9, 0.1]), 0.1));
}

#[test]
fn group_selection_duplicate_reorder_and_delete_preserve_other_objects() {
    let mut p = Page::default();
    let a = p.add(object([10., 10.]));
    let b = p.add(object([10., 10.]));
    let c = p.add(object([10., 10.]));
    assert_eq!(p.hit([10., 10.], 0.), Some(c));
    p.select(Some(a), false);
    p.select(Some(b), true);
    p.group();
    p.select(Some(c), false);
    p.select(Some(a), false);
    assert_eq!(p.selected, vec![a, b]);
    p.reorder(true);
    assert_eq!(p.hit([10., 10.], 0.), Some(b));
    p.duplicate();
    assert_eq!(p.objects.len(), 5);
    assert_eq!(p.selected.len(), 2);
    let duplicates: Vec<_> = p
        .objects
        .iter()
        .filter(|o| p.selected.contains(&o.id))
        .collect();
    assert_eq!(duplicates[0].group, duplicates[1].group);
    assert_ne!(
        duplicates[0].group,
        p.objects.iter().find(|o| o.id == a).unwrap().group
    );
    p.remove();
    assert_eq!(p.objects.len(), 3);
    assert_eq!(
        p.objects.iter().map(|o| o.id).collect::<Vec<_>>(),
        vec![c, a, b]
    );
    p.select(Some(a), false);
    p.ungroup();
    p.select(Some(a), false);
    assert_eq!(p.selected, vec![a]);
}

#[test]
fn undo_redo_restore_entire_document_and_new_edits_discard_redo() {
    let mut d = Document::default();
    d.edit(|p| {
        p.add(object([20., 30.]));
    });
    let first = d.page.clone();
    d.edit(|p| {
        p.size = [148., 210.];
        p.edit_selected(|o| {
            o.rotation = 45.;
            o.fill = Some(0xff0011);
        });
    });
    let second = d.page.clone();
    d.undo();
    assert_eq!(d.page, first);
    d.redo();
    assert_eq!(d.page, second);
    d.undo();
    d.edit(|_| {});
    assert!(d.can_redo(), "no-op must not discard history");
    d.edit(|p| p.remove());
    assert!(!d.can_redo());
    d.undo();
    assert_eq!(d.page, first);
    d.undo();
    assert!(d.page.objects.is_empty());
}

#[test]
fn viewport_mapping_preserves_millimetres_at_fit_and_actual_size() {
    for actual in [false, true] {
        for size in [[800., 600.], [300., 1200.]] {
            let t = ViewTransform::fit([-200., 50.], size, [297., 210.], actual).unwrap();
            for p in [[0., 0.], [100., 77.], [297., 210.]] {
                close(t.world(t.screen(p)), p);
            }
            if actual {
                assert!((t.scale - 96. / 25.4).abs() < 0.0001);
            }
        }
    }
    assert!(ViewTransform::fit([0.; 2], [0., 5.], [297., 210.], false).is_none());
}

#[test]
fn svg_uses_physical_page_dimensions_and_escapes_text_without_editor_handles() {
    let mut p = Page::default();
    p.size = [148., 210.];
    let mut o = Object::new(
        Content::Text("<Hello & \"world\">".into()),
        [60., 50.],
        [80., 20.],
    );
    o.name = "x</title><script>alert(1)</script>".into();
    o.fill = Some(0xff0000);
    o.rotation = 30.;
    p.add(o);
    let svg = p.svg();
    assert!(svg.contains("width=\"148.000mm\" height=\"210.000mm\""));
    assert!(svg.contains("&lt;Hello &amp; &quot;world&quot;&gt;"));
    assert!(!svg.contains("<script>"));
    assert!(!svg.contains("315ed8"));
    assert!(svg.contains("clip-path=\"url(#page)\""));
    let selected_svg = svg;
    p.selected.clear();
    assert_eq!(p.svg(), selected_svg);
}

#[test]
fn data_uris_encode_all_padding_lengths() {
    for (input, expected) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(
            data_uri("image/png", input.as_bytes()),
            format!("data:image/png;base64,{expected}")
        );
    }
}

#[test]
fn editing_nodes_normalizes_bounds_without_moving_the_path() {
    let mut o = object([80., 70.]);
    o.rotation = 37.;
    o.flip = [true, false];
    let points = vec![[-0.5, 0.2], [1., 0.], [1.2, 1.5]];
    let world: Vec<_> = points.iter().map(|&p| o.world(p)).collect();
    o.content = Content::Path {
        points,
        closed: true,
    };
    o.normalize_path();
    let Content::Path { points, .. } = &o.content else {
        panic!("expected path");
    };
    for (point, expected) in points.iter().zip(world) {
        close(o.world(*point), expected);
    }
    assert!(
        points
            .iter()
            .all(|p| p.iter().all(|&v| (0. ..=1.).contains(&v)))
    );
}
