use super::*;
use gpui::{Entity, TestAppContext, VisualTestContext};

fn workspace(cx: &mut TestAppContext) -> (Entity<Workspace>, &mut VisualTestContext) {
    cx.update(crate::menus::init);
    cx.add_window_view(|window, cx| {
        let mut view = Workspace::new(cx);
        view.screen = Screen::Compose;
        window.focus(&view.focus_handle);
        view.render_compose(cx);
        view
    })
}
fn screen(view: &Entity<Workspace>, cx: &VisualTestContext, p: [f32; 2]) -> gpui::Point<Pixels> {
    view.read_with(cx, |v, _| {
        let p = v
            .compose_transform()
            .expect("canvas has a viewport")
            .screen(p);
        point(px(p[0]), px(p[1]))
    })
}
fn add(view: &Entity<Workspace>, cx: &mut VisualTestContext, color: u32) {
    view.update(cx, |v, cx| {
        let mut o = Object::new(Content::Shape(Shape::Rectangle), [80., 60.], [40., 20.]);
        o.fill = Some(color);
        v.compose_edit_page(
            |p| {
                p.add(o);
            },
            cx,
        );
    });
    cx.run_until_parked();
}

#[gpui::test]
fn draw_move_resize_and_undo_through_canvas_events(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.compose_tool = 4;
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    let start = screen(&view, cx, [30., 40.]);
    let end = screen(&view, cx, [90., 70.]);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    view.read_with(cx, |v, _| {
        let o = v.compose.document.page.selection().unwrap();
        assert!((o.size[0] - 60.).abs() < 0.01);
        assert!((o.size[1] - 30.).abs() < 0.01);
        assert!(v.compose.drag.is_none());
    });
    view.update(cx, |v, cx| {
        v.compose_tool = 0;
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    let from = screen(&view, cx, [60., 55.]);
    let to = screen(&view, cx, [80., 65.]);
    cx.simulate_mouse_down(from, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(to, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Default::default());
    view.read_with(cx, |v, _| {
        assert!((v.compose.document.page.selection().unwrap().center[0] - 80.).abs() < 0.01)
    });
    cx.dispatch_action(crate::menus::Undo);
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert!((v.compose.document.page.selection().unwrap().center[0] - 60.).abs() < 0.01)
    });
    cx.dispatch_action(crate::menus::Undo);
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.compose.document.page.objects.is_empty()));
    cx.dispatch_action(crate::menus::Redo);
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.objects.len()),
        1
    );
}

#[gpui::test]
fn fields_accept_native_text_commit_cancel_and_reject_invalid_dimensions(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    cx.run_until_parked();
    let rect = cx.debug_bounds("Width (mm)").unwrap();
    cx.simulate_click(rect.center(), Default::default());
    cx.simulate_input("420");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.size),
        [420., 210.]
    );
    let rect = cx.debug_bounds("Width (mm)").unwrap();
    cx.simulate_click(rect.center(), Default::default());
    cx.simulate_input("NaN");
    cx.simulate_keystrokes("enter");
    assert!(view.read_with(cx, |v, _| v.compose.edit.is_some()));
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.size),
        [420., 210.]
    );
    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |v, _| v.compose.edit.is_none()));
    cx.dispatch_action(crate::menus::Undo);
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.size),
        [297., 210.]
    );
}

#[gpui::test]
fn svg_rendering_preserves_colours_transparency_clipping_and_embedded_vectors(
    cx: &mut TestAppContext,
) {
    let renderer = cx.update(|cx| cx.svg_renderer());
    let mut page = composition::Page::default();
    page.size = [100., 100.];
    let mut o = Object::new(Content::Shape(Shape::Rectangle), [10., 50.], [40., 40.]);
    o.fill = Some(0xff0000);
    page.add(o);
    let inner = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"20\"><rect width=\"20\" height=\"20\" fill=\"#00ff00\"/></svg>";
    let art = Arc::new(Artwork {
        name: "vector".into(),
        origin: Origin::Convert,
        aspect: 1.,
        uri: composition::data_uri("image/svg+xml", inner.as_bytes()),
    });
    page.add(Object::new(Content::Artwork(art), [70., 50.], [20., 20.]));
    let image = raster(&page.svg(), page.size, 100., |i| {
        i.to_image_data(renderer).map_err(|e| e.to_string())
    })
    .unwrap();
    let pixel = |x: usize, y: usize| {
        let bytes = image.as_bytes(0).unwrap();
        let offset = (y * 100 + x) * 4;
        <[u8; 4]>::try_from(&bytes[offset..offset + 4]).unwrap()
    };
    assert_eq!(pixel(10, 50), [0, 0, 255, 255]);
    assert_eq!(pixel(70, 50), [0, 255, 0, 255]);
    assert_eq!(pixel(50, 10), [0, 0, 0, 0]);
    assert_eq!(pixel(99, 99), [0, 0, 0, 0]);
}

#[gpui::test]
fn export_snapshots_document_before_save_dialog_and_cancellation_unblocks_it(
    cx: &mut TestAppContext,
) {
    let (view, cx) = workspace(cx);
    add(&view, cx, 0xff0000);
    let expected = view.read_with(cx, |v, _| v.compose.document.page.svg());
    view.update(cx, |v, cx| v.save_composition(cx));
    assert!(cx.did_prompt_for_new_path());
    add(&view, cx, 0x0000ff);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("page.svg");
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    assert!(view.read_with(cx, |v, _| v.can_export_composition()));
    view.update(cx, |v, cx| v.save_composition(cx));
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.can_export_composition()));
}

#[gpui::test]
fn print_handoff_preserves_page_size_and_ignores_a_replaced_source(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    add(&view, cx, 0xff0000);
    view.update(cx, |v, cx| {
        v.compose.document.page.size = [100., 80.];
        v.compose_send_to_print(cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert!(v.screen == Screen::Print);
        assert!(v.editor.source.is_some());
        assert!(!v.editor.loading && !v.editor.busy);
        assert_eq!(v.editor.params.page_width, 100.);
        assert_eq!(v.editor.params.page_height, 80.);
        assert_eq!(v.editor.params.margin, 0.);
        assert_eq!(v.editor.params.art_width, 100.);
    });
    view.update(cx, |v, cx| {
        v.screen = Screen::Compose;
        v.compose_send_to_print(cx);
        v.editor.load_revision += 1;
        v.editor.loading = false;
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.screen == Screen::Compose));
}

#[gpui::test]
fn imports_embed_pixels_and_library_copies_are_independent(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    image::RgbaImage::from_pixel(8, 4, image::Rgba([255, 0, 0, 128]))
        .save(&path)
        .unwrap();
    view.update(cx, |v, cx| v.load_compose_path(path, cx));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.objects.len()),
        1
    );
    view.update(cx, |v, cx| {
        let art = v.compose.library[0].art.clone();
        v.place_compose_artwork(art, cx);
        v.compose.document.page.edit_selected(|o| o.rotation = 30.);
    });
    view.read_with(cx, |v, _| {
        assert_eq!(v.compose.document.page.objects.len(), 2);
        assert_eq!(v.compose.document.page.objects[0].rotation, 0.);
        if let Content::Artwork(a) = &v.compose.document.page.objects[0].content {
            assert_eq!(a.aspect, 2.);
            assert!(a.uri.starts_with("data:image/png;base64,"));
        } else {
            panic!("missing imported artwork");
        }
    });
}

#[gpui::test]
fn pen_closure_node_edit_and_fill_change_real_geometry(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.compose_tool = 2;
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    for p in [[30., 30.], [90., 30.], [60., 80.], [30., 30.]] {
        let p = screen(&view, cx, p);
        cx.simulate_click(p, Default::default());
    }
    view.read_with(cx, |v, _| { assert!(v.compose.pending.is_empty()); assert!(matches!(&v.compose.document.page.selection().unwrap().content, Content::Path { points, closed: true } if points.len() == 3)); });
    view.update(cx, |v, cx| {
        v.compose_tool = 1;
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    let start = screen(&view, cx, [30., 30.]);
    let end = screen(&view, cx, [20., 25.]);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    view.read_with(cx, |v, _| {
        let o = v.compose.document.page.selection().unwrap();
        let Content::Path { points, .. } = &o.content else {
            panic!("expected path");
        };
        assert!(distance(o.world(points[0]), [20., 25.]) < 0.01);
    });
    view.update(cx, |v, cx| {
        v.compose_tool = 5;
        v.compose.fill = Some(0x00ff00);
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    let p = screen(&view, cx, [60., 45.]);
    cx.simulate_click(p, Default::default());
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.selection().unwrap().fill),
        Some(0x00ff00)
    );
}

#[gpui::test]
fn convert_and_print_snapshots_enter_compose_as_vectors(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    for screen in [Screen::Convert, Screen::Print] {
        view.update(cx, |v, cx| {
            let rgba = image::RgbaImage::from_fn(16, 16, |x, _| {
                if x < 8 {
                    image::Rgba([0, 0, 0, 255])
                } else {
                    image::Rgba([255, 255, 255, 255])
                }
            });
            v.editor.source = Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])));
            v.editor.path = Some(PathBuf::from("test.png"));
            v.screen = screen;
            v.editor.params = effects::Params::default();
            v.send_to_compose(cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(v.screen == Screen::Compose);
            assert_eq!(v.compose.imports, 0);
            let a = v.compose.library.last().unwrap();
            assert!(a.art.uri.starts_with("data:image/svg+xml;base64,"));
            assert_eq!(
                a.art.origin,
                if screen == Screen::Print {
                    Origin::Print
                } else {
                    Origin::Convert
                }
            );
        });
    }
    assert_eq!(
        view.read_with(cx, |v, _| v.compose.document.page.objects.len()),
        2
    );
}

#[gpui::test]
fn text_edits_use_native_input_and_unicode_without_removing_the_object_on_undo(
    cx: &mut TestAppContext,
) {
    let (view, cx) = workspace(cx);
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.compose_tool = 3;
        v.compose_tool_changed(cx);
    });
    cx.run_until_parked();
    let p = screen(&view, cx, [80., 60.]);
    cx.simulate_click(p, Default::default());
    cx.simulate_input("Hello & café 🦀");
    cx.dispatch_action(crate::menus::Undo);
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert_eq!(v.compose.document.page.objects.len(), 1);
        assert_eq!(
            v.compose.document.page.selection().unwrap().content,
            Content::Text("Text".into())
        );
    });
    cx.dispatch_action(crate::menus::Redo);
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert_eq!(
            v.compose.document.page.selection().unwrap().content,
            Content::Text("Hello & café 🦀".into())
        )
    });
}

#[gpui::test]
fn grouped_resize_and_rotation_transform_placements_together(cx: &mut TestAppContext) {
    let (view, cx) = workspace(cx);
    view.update(cx, |v, cx| {
        let page = &mut v.compose.document.page;
        let a = page.add(Object::new(
            Content::Shape(Shape::Rectangle),
            [60., 60.],
            [40., 20.],
        ));
        let b = page.add(Object::new(
            Content::Shape(Shape::Rectangle),
            [120., 60.],
            [40., 20.],
        ));
        page.selected = vec![a, b];
        page.group();
        v.compose_changed(cx);
    });
    cx.run_until_parked();
    let start = screen(&view, cx, [140., 70.]);
    let end = screen(&view, cx, [190., 80.]);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    view.read_with(cx, |v, _| {
        let p = &v.compose.document.page;
        assert!((p.objects[0].center[0] - 30.).abs() < 0.01);
        assert!((p.objects[1].center[0] - 150.).abs() < 0.01);
        assert!((p.objects[0].size[0] - 80.).abs() < 0.01);
    });
    let (start, end) = view.read_with(cx, |v, _| {
        let frame = v.compose.document.page.selection_frame().unwrap();
        let t = v.compose_transform().unwrap();
        let start = frame.world([0.5, -8. / t.scale / frame.size[1]]);
        let radius = distance(start, frame.center);
        let end = [frame.center[0] + radius, frame.center[1]];
        (start, end)
    });
    let start = screen(&view, cx, start);
    let end = screen(&view, cx, end);
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    view.read_with(cx, |v, _| {
        let p = &v.compose.document.page;
        assert!((p.objects[0].center[0] - 90.).abs() < 0.01);
        assert!((p.objects[1].center[0] - 90.).abs() < 0.01);
        assert!((distance(p.objects[0].center, p.objects[1].center) - 120.).abs() < 0.01);
        assert!((p.objects[0].rotation - 90.).abs() < 0.01);
    });
}
