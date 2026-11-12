//! Inspector fields use GPUI's native text input bridge, including IME composition.
use crate::{
    app::Workspace,
    compose_ui::{Edit, Property},
    controls::*,
};
use bitman::composition::Content;
use gpui::{
    Bounds, Context, Div, ElementInputHandler, EntityInputHandler, KeyDownEvent, Pixels, Point,
    Styled, TextRun, UTF16Selection, Window, canvas, div, point, prelude::*, px, rgb,
};
use std::ops::Range;

impl Workspace {
    pub fn begin_compose_input(
        &mut self,
        property: Property,
        value: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(edit) = &mut self.compose.edit
            && edit.property == property
        {
            edit.selection = 0..edit.value.len();
            edit.marked = None;
            cx.notify();
            return;
        }
        if !self.commit_compose_input(cx) {
            return;
        }
        self.compose.edit = Some(Edit {
            property,
            selection: 0..value.len(),
            value,
            marked: None,
        });
        cx.notify();
    }
    pub fn compose_field(
        &self,
        title: &'static str,
        property: Property,
        value: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let editing = self
            .compose
            .edit
            .as_ref()
            .filter(|e| e.property == property);
        let mut field = div()
            .id(title)
            .debug_selector(move || title.into())
            .flex_1()
            .min_w_0()
            .h(px(30.))
            .px(px(8.))
            .border_1()
            .border_color(rgb(if editing.is_some() { 0x315ed8 } else { LINE }))
            .bg(rgb(WHITE))
            .text_xs()
            .overflow_hidden()
            .cursor_text();
        if let Some(edit) = editing {
            let edit = edit.clone();
            let bounds = self.compose.input_bounds.clone();
            let entity = cx.entity();
            let focus = self.focus_handle.clone();
            field = field.child(
                canvas(
                    |_, _, _| (),
                    move |rect, (), window, cx| {
                        *bounds.borrow_mut() = rect;
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(rect, entity.clone()),
                            cx,
                        );
                        let style = window.text_style();
                        let line = window.text_system().shape_line(
                            edit.value.clone().into(),
                            px(12.),
                            &[TextRun {
                                len: edit.value.len(),
                                font: style.font(),
                                color: rgb(INK).into(),
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            }],
                            None,
                        );
                        // Keep the caret visible while entering long text.
                        let caret = line.x_for_index(edit.selection.end);
                        let offset = (caret - rect.size.width + px(4.)).max(px(0.));
                        let origin = rect.origin + point(-offset, px(7.));
                        if !edit.selection.is_empty() {
                            window.paint_quad(gpui::fill(
                                Bounds::from_corners(
                                    origin + point(line.x_for_index(edit.selection.start), px(0.)),
                                    origin + point(line.x_for_index(edit.selection.end), px(15.)),
                                ),
                                gpui::rgba(0x315ed844),
                            ));
                        } else {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    origin + point(caret, px(0.)),
                                    gpui::size(px(1.), px(15.)),
                                ),
                                rgb(0x315ed8),
                            ));
                        }
                        let _ = line.paint(origin, px(16.), window, cx);
                    },
                )
                .size_full(),
            );
        } else {
            field = field.flex().items_center().child(value.clone());
        }
        row()
            .child(label(title).w(px(115.)).flex_shrink_0())
            .child(field.on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    window.focus(&this.focus_handle);
                    this.begin_compose_input(property, value.clone(), cx);
                }),
            ))
    }
    pub fn commit_compose_input(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.compose.edit.clone() else {
            return true;
        };
        let text = edit.value.trim();
        let color = if matches!(edit.property, Property::Fill | Property::Stroke) {
            if text.eq_ignore_ascii_case("none") {
                Some(None)
            } else {
                let hex = text.trim_start_matches('#');
                if hex.len() == 6 {
                    u32::from_str_radix(hex, 16).ok().map(Some)
                } else {
                    None
                }
            }
        } else {
            None
        };
        let value = text.parse::<f32>().ok().filter(|v| v.is_finite());
        let valid = match edit.property {
            Property::Text => edit.value.chars().count() <= 1000,
            Property::Fill | Property::Stroke => color.is_some(),
            Property::PageWidth | Property::PageHeight => {
                value.is_some_and(|v| (20. ..=2000.).contains(&v))
            }
            Property::Width | Property::Height => value.is_some_and(|v| (0.1..=4000.).contains(&v)),
            Property::StrokeWidth => value.is_some_and(|v| (0. ..=100.).contains(&v)),
            _ => value.is_some_and(|v| (-10000. ..=10000.).contains(&v)),
        };
        if !valid {
            self.compose.status = match edit.property {
                Property::Fill | Property::Stroke => "Use a six-digit hex colour or none",
                Property::PageWidth | Property::PageHeight => {
                    "Page dimensions must be between 20 and 2000 mm"
                }
                Property::Text => "Text is limited to 1000 characters",
                _ => "Enter a finite number within the field's range",
            }
            .into();
            cx.notify();
            return false;
        }
        self.compose.document.checkpoint();
        let page = &mut self.compose.document.page;
        let v = value.unwrap_or(0.);
        match edit.property {
            Property::PageWidth => page.size[0] = v,
            Property::PageHeight => page.size[1] = v,
            Property::Fill => {
                let c = color.unwrap();
                self.compose.fill = c;
                page.edit_selected(|o| o.fill = c);
            }
            Property::Stroke => {
                let c = color.unwrap();
                self.compose.stroke = c;
                page.edit_selected(|o| o.stroke = c);
            }
            Property::StrokeWidth => {
                self.compose.stroke_width = v;
                page.edit_selected(|o| o.stroke_width = v);
            }
            Property::Text => page.edit_selected(|o| {
                if let Content::Text(text) = &mut o.content {
                    *text = edit.value.clone();
                }
            }),
            prop => page.edit_selected(|o| match prop {
                Property::X => o.center[0] = v,
                Property::Y => o.center[1] = v,
                Property::Width => o.size[0] = v,
                Property::Height => o.size[1] = v,
                Property::Rotation => o.rotation = v.rem_euclid(360.),
                _ => {}
            }),
        }
        self.compose.edit = None;
        self.compose.status = "Updated composition".into();
        self.compose_changed(cx);
        true
    }
    pub fn compose_input_delete(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(e) = &mut self.compose.edit {
            if e.selection.is_empty() {
                if forward {
                    e.selection.end += e.value[e.selection.end..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8);
                } else {
                    e.selection.start = e.value[..e.selection.start]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                }
            }
            e.value.replace_range(e.selection.clone(), "");
            e.selection.end = e.selection.start;
            e.marked = None;
            cx.notify();
        }
    }
    pub fn compose_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if self.compose.edit.is_some() {
            match key {
                "enter" => {
                    self.commit_compose_input(cx);
                }
                "escape" => {
                    self.compose.edit = None;
                    cx.notify();
                }
                "backspace" => self.compose_input_delete(false, cx),
                "delete" => self.compose_input_delete(true, cx),
                "left" | "right" | "home" | "end" => {
                    let e = self.compose.edit.as_mut().unwrap();
                    let offset = match key {
                        "home" => 0,
                        "end" => e.value.len(),
                        "left" => e.value[..e.selection.start]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i),
                        _ => {
                            e.selection.end
                                + e.value[e.selection.end..]
                                    .chars()
                                    .next()
                                    .map_or(0, char::len_utf8)
                        }
                    };
                    e.selection = offset..offset;
                    e.marked = None;
                    cx.notify();
                }
                _ => return,
            }
            cx.stop_propagation();
            return;
        }
        match key {
            "escape" => {
                self.end_compose_drag();
                self.compose.pending.clear();
                self.compose.document.page.selected.clear();
                self.compose.node = None;
                cx.notify();
            }
            "enter" if self.compose_tool == 2 => self.compose_finish_path(false, cx),
            "left" | "right" | "up" | "down"
                if !event.keystroke.modifiers.platform && !event.keystroke.modifiers.control =>
            {
                self.compose.document.checkpoint();
                let step = if event.keystroke.modifiers.shift {
                    10.
                } else {
                    1.
                };
                self.compose.document.page.edit_selected(|o| match key {
                    "left" => o.center[0] -= step,
                    "right" => o.center[0] += step,
                    "up" => o.center[1] -= step,
                    _ => o.center[1] += step,
                });
                self.compose_changed(cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
    pub fn compose_select_all(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = &mut self.compose.edit {
            e.selection = 0..e.value.len();
        } else {
            self.compose.document.page.selected = self
                .compose
                .document
                .page
                .objects
                .iter()
                .map(|o| o.id)
                .collect();
        }
        cx.notify();
    }
    pub fn compose_clipboard(&mut self, cut: bool, cx: &mut Context<Self>) {
        if let Some(e) = &self.compose.edit
            && !e.selection.is_empty()
        {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                e.value[e.selection.clone()].into(),
            ));
            if cut {
                self.compose_input_delete(false, cx);
            }
        }
    }
    pub fn compose_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            self.replace_text_in_range(None, &text.replace(['\n', '\r'], " "), window, cx);
        }
    }
}
fn byte_offset(s: &str, utf16: usize) -> usize {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if n >= utf16 {
            return i;
        }
        n += c.len_utf16();
    }
    s.len()
}
fn utf16_range(s: &str, r: &Range<usize>) -> Range<usize> {
    s[..r.start].encode_utf16().count()..s[..r.end].encode_utf16().count()
}
fn byte_range(s: &str, r: Range<usize>) -> Range<usize> {
    byte_offset(s, r.start)..byte_offset(s, r.end)
}
impl EntityInputHandler for Workspace {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let e = self.compose.edit.as_ref()?;
        let r = byte_range(&e.value, r);
        *actual = Some(utf16_range(&e.value, &r));
        Some(e.value[r].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let e = self.compose.edit.as_ref()?;
        Some(UTF16Selection {
            range: utf16_range(&e.value, &e.selection),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let e = self.compose.edit.as_ref()?;
        e.marked.as_ref().map(|r| utf16_range(&e.value, r))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if let Some(e) = &mut self.compose.edit {
            e.marked = None;
        }
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(e) = &mut self.compose.edit {
            let r = r
                .map(|r| byte_range(&e.value, r))
                .or(e.marked.clone())
                .unwrap_or(e.selection.clone());
            let text = text.replace(['\n', '\r'], " ");
            if e.value.len() - r.len() + text.len() > 8192 {
                return;
            }
            e.value.replace_range(r.clone(), &text);
            let offset = r.start + text.len();
            e.selection = offset..offset;
            e.marked = None;
            cx.notify();
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = &self.compose.edit else {
            return;
        };
        let start = r
            .as_ref()
            .map(|r| byte_offset(&e.value, r.start))
            .or(e.marked.as_ref().map(|r| r.start))
            .unwrap_or(e.selection.start);
        self.replace_text_in_range(r, text, window, cx);
        if let Some(e) = &mut self.compose.edit {
            let end = (start + text.len()).min(e.value.len());
            if !e.value.is_char_boundary(start) || !e.value.is_char_boundary(end) {
                return;
            }
            e.marked = Some(start..end);
            if let Some(r) = selection {
                let r = byte_range(&e.value[start..end], r);
                e.selection = start + r.start..start + r.end;
            }
        }
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.compose.edit.as_ref()?;
        Some(*self.compose.input_bounds.borrow())
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let e = self.compose.edit.as_ref()?;
        Some(e.value[..e.selection.end].encode_utf16().count())
    }
}
