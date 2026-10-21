// SPDX-License-Identifier: GPL-3.0-or-later
//! Self-contained, antialiased power badge in StatusNotifier ARGB byte order.
//! No desktop icon theme, filesystem asset or external renderer is required.
pub fn pixels(size: u32, color: [u8; 3]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0u32; 4];
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f64 + (sx as f64 + 0.5) / 4.) / size as f64 - 0.5;
                    let py = (y as f64 + (sy as f64 + 0.5) / 4.) / size as f64 - 0.5;
                    if px.hypot(py) > 0.47 {
                        continue;
                    }
                    let ring = (px.hypot(py - 0.035) - 0.25).abs() < 0.045
                        && !(py < -0.10 && px.abs() < 0.13);
                    let stem = px.abs() < 0.045 && (-0.29..=0.04).contains(&py);
                    let rgb = if ring || stem { [255, 255, 255] } else { color };
                    sum[0] += 255;
                    for i in 0..3 {
                        sum[i + 1] += rgb[i] as u32;
                    }
                }
            }
            bytes.push((sum[0] / 16) as u8);
            for channel in &sum[1..] {
                bytes.push((channel * 255).checked_div(sum[0]).unwrap_or(0) as u8);
            }
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    #[test]
    fn pixmaps_use_network_argb_with_transparency_and_visible_badge() {
        for size in [16, 22, 24, 32, 48] {
            let pixels = super::pixels(size, [71, 120, 255]);
            assert_eq!(pixels.len(), (size * size * 4) as usize);
            assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
            assert!(pixels.as_chunks::<4>().0.contains(&[255, 71, 120, 255]));
            assert!(pixels.as_chunks::<4>().0.contains(&[255, 255, 255, 255]));
        }
    }
}
