//! Local font faces, embedded in the executable and registered before rendering.

use std::borrow::Cow;

use gpui::App;

pub const BODY: &str = "Sometype Mono";
pub const DISPLAY: &str = "Departure Mono";

pub const LICENSES: &str = concat!(
    "Departure Mono\n\n",
    include_str!("../assets/fonts/departure-mono/OFL.txt"),
    "\n\nSometype Mono\n\n",
    include_str!("../assets/fonts/sometype-mono/OFL.txt"),
);

pub fn load(cx: &App) {
    let faces: &[&'static [u8]] = &[
        include_bytes!("../assets/fonts/departure-mono/DepartureMono-1.500.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-Regular.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-RegularItalic.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-Medium.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-MediumItalic.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-Bold.woff2"),
        include_bytes!("../assets/fonts/sometype-mono/SometypeMono-BoldItalic.woff2"),
    ];
    cx.text_system()
        .add_fonts(faces.iter().map(|&face| Cow::Borrowed(face)).collect())
        .expect("could not load the embedded Bitman fonts");
}
