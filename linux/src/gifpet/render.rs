//! 两个后端共用的预乘缩放和命中轮廓；每个源帧只计算一次。
use std::collections::HashMap;
use std::sync::Arc;
use super::decode::Rgba;

pub const WIDTH: u16 = 155;
pub const HEIGHT: u16 = 168;

pub struct Frame {
    pub pixels: Vec<u8>,
    pub spans: Vec<(i32, i32, i32)>,
}

#[derive(Default)]
pub struct FrameCache(HashMap<usize, Frame>);
impl FrameCache {
    pub fn get(&mut self, source: &Arc<Rgba>) -> &Frame {
        self.0.entry(Arc::as_ptr(source) as usize).or_insert_with(|| scale(source))
    }
}

fn scale(source: &Rgba) -> Frame {
    let mut pixels = vec![0; WIDTH as usize * HEIGHT as usize * 4];
    for y in 0..HEIGHT as usize {
        let sy = ((y as f64 + 0.5) * source.h as f64 / HEIGHT as f64 - 0.5).max(0.0);
        let y0 = (sy.floor() as u32).min(source.h - 1);
        let y1 = (y0 + 1).min(source.h - 1);
        let fy = sy - y0 as f64;
        for x in 0..WIDTH as usize {
            let sx = ((x as f64 + 0.5) * source.w as f64 / WIDTH as f64 - 0.5).max(0.0);
            let x0 = (sx.floor() as u32).min(source.w - 1);
            let x1 = (x0 + 1).min(source.w - 1);
            let fx = sx - x0 as f64;
            for c in 0..4 {
                let p = |x: u32, y: u32| source.premul_bgra[((y * source.w + x) * 4) as usize + c] as f64;
                pixels[(y * WIDTH as usize + x) * 4 + c] =
                    ((p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx) * (1.0 - fy)
                    + (p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx) * fy).round() as u8;
            }
        }
    }
    let mut spans = Vec::new();
    for y in 0..HEIGHT as usize {
        let mut x = 0;
        while x < WIDTH as usize {
            if pixels[(y * WIDTH as usize + x) * 4 + 3] < 128 { x += 1; continue; }
            let start = x;
            while x < WIDTH as usize && pixels[(y * WIDTH as usize + x) * 4 + 3] >= 128 { x += 1; }
            spans.push((start as i32, y as i32, (x - start) as i32));
        }
    }
    Frame { pixels, spans }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonsquare_frame_keeps_full_height_and_alpha() {
        let f = scale(&Rgba { w: 1, h: 2, premul_bgra: vec![0, 0, 255, 255, 0, 0, 0, 0] });
        assert_eq!(f.pixels.len(), 155 * 168 * 4);
        assert_eq!(&f.pixels[..4], &[0, 0, 255, 255]);
        assert_eq!(&f.pixels[f.pixels.len()-4..], &[0, 0, 0, 0]);
        assert!(f.spans.iter().all(|&(_, y, _)| y < 84));
    }
}
