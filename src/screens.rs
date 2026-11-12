use gpui::{Context, Div, Pixels, Styled, div, prelude::*, px, rgb};

use crate::{
    app::{Screen, Workspace},
    controls::*,
};

impl Workspace {
    pub fn help_sidebar(&self, cx: &mut Context<Self>) -> Div {
        column()
            .gap_0()
            .child(section("Updates", self.update_controls(cx)))
            .child(section(
                "Getting started",
                column()
                    .child(muted("01   Convert a raster image"))
                    .child(muted("02   Prepare print plates"))
                    .child(muted("03   Export an SVG")),
            ))
            .child(section(
                "Supported images",
                column().child(muted("PNG · JPG · GIF · WebP · BMP · TIFF")),
            ))
            .child(section(
                "Original project",
                column()
                    .child(label("Boot Tool"))
                    .child(muted("By sshhiiff and contributors"))
                    .child(muted("Credits and source links in the guide.")),
            ))
    }

    fn help_credits(&self) -> Div {
        help_section(
            "Credits — original implementation",
            column()
                .w_full()
                .min_w_0()
                .flex_shrink_0()
                .text_sm()
                .text_color(rgb(MUTED))
                .children([
                    "Bitman is based on Boot Tool by sshhiiff and its contributors. Credit for the original application, its image-to-vector and screen-printing workflows, workspace design, interface layout, and colour palette belongs to the Boot Tool project.",
                    "Boot Tool's Convert, Compose, Print, and Simulate workspaces provided the reference for this app. This adaptation references Boot Tool 0.8.8 at revision 5bd1a4303093d01bbab3d1f5b61cbc8f19699e3a.",
                    "Bitman reimplements the interface in Rust with GPUI and provides its own Rust effects engine. The original Boot Tool uses Python and a browser interface, including Paper.js for Compose. Its Python code and Paper.js are not bundled in Bitman.",
                ].into_iter().map(help_text))
                .child(row().w_full().min_w_0().flex_shrink_0().flex_wrap().children([
                    ("Boot Tool source ↗", "https://github.com/sshhiiff/Boot-Tool"),
                    ("sshhiiff ↗", "https://github.com/sshhiiff"),
                    ("Contributors ↗", "https://github.com/sshhiiff/Boot-Tool/graphs/contributors"),
                    ("Referenced version ↗", "https://github.com/sshhiiff/Boot-Tool/tree/5bd1a4303093d01bbab3d1f5b61cbc8f19699e3a"),
                ].into_iter().map(|(title, url)| {
                    button(title)
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .id(title)
                        .cursor_pointer()
                        .on_click(move |_, _, cx| cx.open_url(url))
                })))
                .child(help_text("Additional credits: GPUI by Zed Industries; Departure Mono by Helena Zhang; Sometype Mono by Ryoichi Tsunekawa / Dharma Type. Both fonts use the SIL Open Font License 1.1.")),
        )
        .w_full()
        .flex_shrink_0()
    }

    pub fn stage(&self, available_width: Pixels, cx: &mut Context<Self>) -> Div {
        let help_width = (available_width - px(80.0)).clamp(px(0.0), px(790.0));
        let stage = div().flex().flex_col().flex_1().min_h_0().overflow_hidden();
        if matches!(self.screen, Screen::Convert | Screen::Print)
            && self.editor.source.is_some()
            && (self.screen == Screen::Print || self.preview != 4)
        {
            return self.image_stage(cx);
        }
        match self.screen {
            Screen::Convert if self.preview == 4 => stage.child(self.batch_stage()),
            Screen::Convert => stage.items_center().justify_center().child(
                column().items_center().gap(px(22.0)).p(px(48.0))
                    .child(div().size(px(54.0)).rounded(px(12.0)).bg(rgb(BUTTON)).flex().items_center().justify_center().text_2xl().text_color(rgb(ACCENT)).child("↥"))
                    .child(empty_message("Drop an image to get started", "or open an image from your computer"))
                    .child(muted("PNG, JPG, GIF, WebP, BMP, TIFF"))
                    .child(button("Choose image…").id("choose-image").cursor_pointer().on_click(cx.listener(|this, _, _, cx|this.open_image(cx))))),
            Screen::Compose => self.compose_stage(cx),
            Screen::Print => stage.items_center().justify_center().child(
                column().items_center().gap(px(20.0))
                    .child(div().w(px(480.0)).h(px(340.0)).bg(rgb(WHITE)).border_1().border_color(rgb(LINE)).shadow_sm()
                        .flex().items_center().justify_center()
                        .child(empty_message("Your print starts here", "Open an image to preview the separated plates")))
                    .child(muted("A4 · 297 × 210 mm · Landscape"))),
            Screen::Simulate => self.simulation_stage(cx),
            // Direct children let GPUI measure the scroll range through the final section.
            // Resolve the width before text measurement: max-width can wrap text after
            // section heights are measured, leaving borders across the last line.
            Screen::Help => stage.child(
                div().id("help-content").flex().flex_col().flex_1().w_full().min_w_0().min_h_0().overflow_y_scroll().p(px(40.0)).gap(px(24.0))
                    .child(heading("A workspace for images and print").flex().flex_col().w_full().min_w_0().flex_shrink_0().text_2xl().w(help_width))
                    .child(help_text("Open an image, adjust its appearance, and export vectors or print plates. All processing stays on your Mac.").text_xs().w(help_width))
                    .child(self.help_credits().w(help_width))
                    .children([
                        ("Open an image", "Choose File → Open Image… (⌘O), or drop an image into the window. PNG, JPEG, GIF, WebP, BMP, and TIFF are supported."),
                        ("Convert", "Prepare raster images for vector tracing. Adjust the look, threshold, colours, and detail. Compare the SVG with the original using the Original, Split, or Overlay view."),
                        ("Print", "Choose one ink, CMYK, or spot colours. Set levels, halftone or dither options, page dimensions, and registration marks. Review each plate before export."),
                        ("Export", "Choose File → Export SVG… (⇧⌘S) from Compose, or from Convert / Print after processing finishes. Print exports layered SVG plates. Convert / Print previews are capped at 1024 pixels per side; their exports are capped at 4096. Compose exports vector geometry without a raster cap."),
                        ("Keyboard shortcuts", "⌘O Open image · ⇧⌘S Export SVG · ⌘1 Convert · ⌘2 Print · ⌘3 Simulate · ⌘4 Compose · ⌘Z / ⇧⌘Z Undo / redo in Compose · ⌥⌘S Save mock-up · Delete/Backspace Remove selected artwork · ⌃⌘F Full screen · ⌘M Minimize · ⇧⌘/ Help · ⌘W Close window · ⌘Q Quit"),
                        ("Simulate", "Add raster artwork from a file or use Add to Simulate in Convert / Print. Click a library thumbnail to place another copy. Drag designs or use placement sliders and step buttons; the lower blue handle resizes and the upper handle rotates. Front and back keep separate designs. Select a design and press Delete or Backspace, choose Edit → Remove Artwork, or use its Remove button; the library copy stays available. Fit and 100% control the preview. JPG export saves the current side without selection handles. Garment dimensions are illustrative, not manufacturer specifications."),
                        ("Compose", "Draw rectangles, ellipses, polygons or stars; click to build straight paths. Add text and edit fields with Enter to apply or Esc to cancel. Import images, or use Add to Compose in Convert / Print. Shift-click layers or objects to select several; group, duplicate, reorder and transform them. Drag blue handles to resize or rotate. Nodes edits polygon and path vertices. Save SVG embeds artwork; Send to Print rasterizes the current page at up to 4096 pixels. Undo / redo and Fit / 100% are supported. Boolean path operations and curve editing are not available."),
                        ("Still in development", "Batch conversion and Fit / 100% in Convert and Print are layout previews."),
                    ].into_iter().map(|(title, detail)| help_section(title, help_text(detail)).w(help_width))),
            ),
        }
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
                    .child(heading("Batch conversion"))
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

// Flex columns give GPUI text a definite wrapping width. Block text containers can
// retain a wider intrinsic measurement, overflowing the guide's capped width.
fn help_text(text: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .text_sm()
        .text_color(rgb(MUTED))
        .child(text)
}

fn help_section(title: &'static str, content: impl IntoElement) -> Div {
    column()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .gap(px(14.0))
        .p(px(16.0))
        .border_b_1()
        .border_color(rgb(LINE))
        .child(
            heading(title)
                .flex()
                .flex_col()
                .w_full()
                .min_w_0()
                .flex_shrink_0()
                .text_sm()
                .px(px(12.0))
                .py(px(6.0))
                .bg(rgb(ACCENT))
                .text_color(rgb(WHITE)),
        )
        .child(content)
}
