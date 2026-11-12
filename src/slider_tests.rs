use crate::{
    app::{Screen, Workspace},
    effect_ui::Number,
};
use bitman::effects::Params;
use gpui::{
    Bounds, Entity, MouseButton, Pixels, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, point, px,
};

fn scroll_to(title: &'static str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let bottom = cx.update(|window, _| window.viewport_size().height - px(80.0));
    for _ in 0..4 {
        let rect = cx
            .debug_bounds(title)
            .unwrap_or_else(|| panic!("missing {title}"));
        if rect.top() >= px(120.0) && rect.bottom() < bottom {
            return rect;
        }
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(150.0), px(300.0)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(220.0) - rect.top())),
            ..Default::default()
        });
    }
    panic!("could not scroll to {title}");
}

fn assert_slider(
    view: &Entity<Workspace>,
    title: &'static str,
    key: Number,
    cx: &mut VisualTestContext,
) {
    let rect = scroll_to(title, cx);
    for (fraction, down) in [(0.6, true), (0.8, false)] {
        let before = view.read_with(cx, |view, _| view.editor.params);
        let position = point(
            rect.left() + rect.size.width * fraction,
            rect.bottom() - px(7.0),
        );
        if down {
            cx.simulate_mouse_down(position, MouseButton::Left, Default::default());
        } else {
            cx.simulate_mouse_move(position, MouseButton::Left, Default::default());
        }
        let (_, min, max, step, _) = key.spec(&before);
        let mut expected = before;
        key.set(
            &mut expected,
            (((min + fraction * (max - min)) / step).round() * step).clamp(min, max),
        );
        expected.black = expected.black.min(expected.white - 0.004);
        expected.white = expected.white.max(expected.black + 0.004);
        let actual = view.read_with(cx, |view, _| view.editor.params);
        assert_eq!(
            actual, expected,
            "{title}: click/drag changed the wrong value"
        );
        if !down {
            cx.simulate_mouse_up(position, MouseButton::Left, Default::default());
            assert!(
                view.read_with(cx, |view, _| view.editor.active_slider.is_none()),
                "{title}: drag did not end"
            );
        }
    }
}

#[gpui::test]
fn all_effect_sliders_update_their_own_values(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Workspace::new(cx));
    let convert = [
        ("Scale", Number::Upscale),
        ("Threshold (zero = auto)", Number::Threshold),
        ("Speckle removal", Number::Speckle),
        ("Edge smoothing", Number::Edge),
        ("Simplify", Number::Epsilon),
        ("Corner sharpness", Number::Corner),
        ("Colours", Number::Colors),
    ];
    let print = [
        ("Black point", Number::Black),
        ("Midtones", Number::Gamma),
        ("White point", Number::White),
        ("Frequency", Number::Lpi),
        ("Angle", Number::Angle),
        ("Output resolution", Number::Ppi),
        ("Contrast", Number::Contrast),
        ("Brightness", Number::Brightness),
        ("Soften before screening", Number::Smoothing),
        ("Outline", Number::Outline),
        ("Artwork width (zero = fit)", Number::ArtWidth),
        ("Artwork margin", Number::Margin),
    ];
    for (screen, method, render, kind, sliders) in [
        (Screen::Convert, 0, 0, 0, convert.as_slice()),
        (Screen::Print, 0, 0, 0, print.as_slice()),
        (
            Screen::Print,
            1,
            0,
            0,
            &[("Black amount", Number::BlackAmount)],
        ),
        (
            Screen::Print,
            2,
            0,
            0,
            &[("Number of inks", Number::Inks), ("Trap", Number::Trap)],
        ),
        (
            Screen::Print,
            2,
            1,
            1,
            &[
                ("Dither size", Number::Grain),
                ("Pattern size", Number::PatternSize),
                ("Solid detection", Number::Solid),
            ],
        ),
    ] {
        view.update(cx, |view, cx| {
            view.screen = screen;
            view.print_method = method;
            view.print_render = render;
            view.print_kind = kind;
            view.dither = 5;
            view.editor.params = Params::default();
            view.request_effects(cx);
        });
        cx.run_until_parked();
        for &(title, key) in sliders {
            assert_slider(&view, title, key, cx);
        }
    }
}

#[gpui::test]
fn page_buttons_do_not_change_dither_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut view = Workspace::new(cx);
        view.screen = Screen::Print;
        view.print_render = 1;
        view.request_effects(cx);
        view
    });
    assert_slider(&view, "Dither size", Number::Grain, cx);
    for (title, width, height) in [
        ("A5", 210.0, 148.0),
        ("A3", 420.0, 297.0),
        ("A4", 297.0, 210.0),
    ] {
        let rect = scroll_to(title, cx);
        let mut expected = view.read_with(cx, |view, _| view.editor.params);
        expected.page_width = width;
        expected.page_height = height;
        cx.simulate_click(rect.center(), Default::default());
        assert_eq!(
            view.read_with(cx, |view, _| view.editor.params),
            expected,
            "{title} changed unrelated settings"
        );
    }
}

#[gpui::test]
fn releasing_outside_then_clicking_page_does_not_resume_a_slider(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut view = Workspace::new(cx);
        view.screen = Screen::Print;
        view.print_render = 1;
        view.request_effects(cx);
        view
    });
    for deliver_release in [true, false] {
        let rect = scroll_to("Dither size", cx);
        cx.simulate_mouse_down(rect.center(), MouseButton::Left, Default::default());
        if deliver_release {
            cx.simulate_mouse_up(
                point(px(-10.0), px(-10.0)),
                MouseButton::Left,
                Default::default(),
            );
            assert!(view.read_with(cx, |view, _| view.editor.active_slider.is_none()));
        }
        let mut expected = view.read_with(cx, |view, _| view.editor.params);
        expected.page_width = 210.0;
        expected.page_height = 148.0;
        let rect = scroll_to("A5", cx);
        cx.simulate_mouse_down(rect.center(), MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            rect.center() + point(px(1.0), px(0.0)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(rect.center(), MouseButton::Left, Default::default());
        assert_eq!(
            view.read_with(cx, |view, _| view.editor.params),
            expected,
            "page click resumed the old dither drag"
        );
    }
}

#[gpui::test]
fn all_placement_sliders_update_their_own_values(cx: &mut TestAppContext) {
    use bitman::simulation::{Artwork, Origin};
    use std::sync::Arc;

    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut view = Workspace::new(cx);
        view.screen = Screen::Simulate;
        view.simulation.scene.add(Arc::new(
            Artwork::from_bgra("test".into(), Origin::File, 2, 1, &[255; 8]).unwrap(),
        ));
        view
    });
    for locked in [false, true] {
        view.update(cx, |view, cx| {
            view.simulation
                .scene
                .selection_mut()
                .unwrap()
                .keep_proportions = locked;
            cx.notify();
        });
        cx.run_until_parked();
        for (index, (title, min, max, step)) in [
            ("Width", 0.5, 100.0, 0.5),
            ("Height", 0.5, 100.0, 0.5),
            ("From collar", -20.0, 90.0, 0.5),
            ("From centre", -70.0, 70.0, 0.5),
            ("Rotation", -180.0, 180.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = scroll_to(title, cx);
            for (fraction, down) in [(0.3_f32, true), (0.4, false)] {
                let mut expected = view.read_with(cx, |view, _| {
                    let d = view.simulation.scene.selection().unwrap();
                    [d.width, d.height, d.top, d.x, d.rotation]
                });
                let value = ((min + fraction * (max - min)) / step).round() * step;
                if locked && index < 2 {
                    let ratio = expected[1] / expected[0];
                    expected[1 - index] = if index == 0 {
                        value * ratio
                    } else {
                        value / ratio
                    };
                }
                expected[index] = value;
                let position = point(
                    rect.left() + rect.size.width * fraction,
                    rect.bottom() - px(7.0),
                );
                if down {
                    cx.simulate_mouse_down(position, MouseButton::Left, Default::default());
                } else {
                    cx.simulate_mouse_move(position, MouseButton::Left, Default::default());
                }
                let actual = view.read_with(cx, |view, _| {
                    let d = view.simulation.scene.selection().unwrap();
                    [d.width, d.height, d.top, d.x, d.rotation]
                });
                for (actual, expected) in actual.into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() < 0.001,
                        "{title}: expected {expected}, got {actual}"
                    );
                }
                if !down {
                    cx.simulate_mouse_up(position, MouseButton::Left, Default::default());
                    assert!(view.read_with(cx, |view, _| view.simulation.active_slider.is_none()));
                }
            }
        }
    }
}
