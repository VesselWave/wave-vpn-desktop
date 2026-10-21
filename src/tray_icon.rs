// SPDX-License-Identifier: GPL-3.0-or-later
//! Authored Wave shield/wave asset rendered to StatusNotifier network ARGB.
//! See docs/PROVENANCE.md for the original presentation asset.
pub fn pixels(size: u32, color: [u8; 3]) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    let svg = include_str!("../resources/wave-vpn-tray.svg").replace(
        "#2F6BFF",
        &format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]),
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .expect("bundled Wave tray SVG must be valid");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("valid tray dimensions");
    let scale = size as f32 / tree.size().height();
    let x = (size as f32 - tree.size().width() * scale) / 2.;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(x, 0.),
        &mut pixmap.as_mut(),
    );
    // tiny-skia supplies premultiplied RGBA; StatusNotifier expects straight ARGB.
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let rgba = pixel.demultiply();
            [rgba.alpha(), rgba.red(), rgba.green(), rgba.blue()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn pixmaps_use_network_argb_with_transparency_shield_and_white_wave() {
        for size in [16, 22, 24, 32, 48] {
            for color in [
                [71, 120, 255],
                [220, 69, 77],
                [210, 143, 27],
                [104, 112, 128],
            ] {
                let pixels = super::pixels(size, color);
                assert_eq!(pixels.len(), (size * size * 4) as usize);
                assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
                assert!(pixels
                    .as_chunks::<4>()
                    .0
                    .contains(&[255, color[0], color[1], color[2]]));
                assert!(pixels.as_chunks::<4>().0.contains(&[255, 255, 255, 255]));
            }
        }
    }
}
