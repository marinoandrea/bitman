//! Generate deterministic effect fixtures without a UI or any image codec dependency.
use bitman::effects::*;
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/effect-gallery"));
    std::fs::create_dir_all(&dir)?;
    let (w, h) = (256, 192);
    let mut pixels = vec![[255u8; 4]; w * h];
    for y in 12..h - 12 {
        for x in 12..w - 12 {
            let rgb = if y > 126 {
                let v = ((x - 12) * 255 / (w - 24)) as u8;
                [v; 3]
            } else if (x as isize - 70).pow(2) + (y as isize - 65).pow(2) < 38 * 38 {
                if (x as isize - 70).pow(2) + (y as isize - 65).pow(2) < 18 * 18 {
                    [255; 3]
                } else {
                    [0; 3]
                }
            } else if (130..174).contains(&x) {
                [220, 45, 55]
            } else if (184..228).contains(&x) {
                [20, 115, 210]
            } else {
                [255; 3]
            };
            pixels[y * w + x] = [rgb[2], rgb[1], rgb[0], 255];
        }
    }
    let image = ImageView::new(pixels.as_flattened(), w, h, w * 4)?;
    let mut engine = Engine::default();
    let base = Params {
        max_side: 512,
        denoise: false,
        speckle: 0,
        edge: 0.0,
        epsilon: 0.1,
        ..Params::default()
    };
    let cases = [
        (
            "lineart",
            Params {
                mode: Mode::Lineart,
                ..base
            },
        ),
        (
            "colour",
            Params {
                mode: Mode::Color,
                ..base
            },
        ),
        (
            "one-screen",
            Params {
                operation: Operation::Print,
                ..base
            },
        ),
        (
            "cmyk-screen",
            Params {
                operation: Operation::Print,
                method: Method::Cmyk,
                ..base
            },
        ),
        (
            "spot-flat",
            Params {
                operation: Operation::Print,
                method: Method::Spot,
                ..base
            },
        ),
        (
            "spot-photo",
            Params {
                operation: Operation::Print,
                method: Method::Spot,
                photo: true,
                keep_solid: true,
                ..base
            },
        ),
        (
            "dark-flat",
            Params {
                operation: Operation::Print,
                method: Method::Spot,
                dark: true,
                dark_invert: true,
                underbase: true,
                ..base
            },
        ),
        (
            "dark-one",
            Params {
                operation: Operation::Print,
                dark: true,
                dark_invert: true,
                underbase: true,
                ink: [255; 3],
                ..base
            },
        ),
    ];
    let dithers = [
        Dither::Floyd,
        Dither::Atkinson,
        Dither::Stucki,
        Dither::Bayer,
        Dither::Blue,
        Dither::Serpentine,
        Dither::Random,
    ];
    for (name, p) in
        cases
            .into_iter()
            .map(|(n, p)| (n.to_string(), p))
            .chain(dithers.into_iter().map(|d| {
                (
                    format!("dither-{d:?}"),
                    Params {
                        operation: Operation::Print,
                        render: Render::Dither,
                        dither: d,
                        ..base
                    },
                )
            }))
    {
        engine.render(image, p)?;
        engine.encode_bmp();
        engine.write_svg(p, true);
        std::fs::write(dir.join(format!("{name}.bmp")), &engine.bmp)?;
        std::fs::write(dir.join(format!("{name}.svg")), &engine.svg)?;
        println!(
            "{name}: {} × {}, {} plates, {} paths",
            engine.info.width, engine.info.height, engine.info.plates, engine.info.paths
        );
    }
    Ok(())
}
