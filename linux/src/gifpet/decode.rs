//! GIF 解码：解出逐帧位图与逐帧延时。
//! 字节序固定为预乘 alpha 的 B,G,R,A——小端下与 X11 32bpp ZPixmap 和
//! Wayland Argb8888 完全一致，一份缓冲同时喂两个后端，零转换。

use std::path::Path;
use std::sync::Arc;

/// 一帧解好的位图。
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    /// 预乘 alpha 的 B,G,R,A 字节序。
    pub premul_bgra: Vec<u8>,
}

/// 一段动画：逐帧位图 + 逐帧延时（毫秒）+ 是否循环。
pub struct Clip {
    pub frames: Vec<Arc<Rgba>>,
    pub delays_ms: Vec<u32>,
    pub loops: bool,
}

/// 读取并解码一个 GIF；文件缺失、解码失败或无帧一律返回 None（该姿势没有动画）。
pub fn load_clip(path: &Path, loops: bool) -> Option<Clip> {
    let file = std::fs::File::open(path).ok()?;
    let mut options = gif::DecodeOptions::new();
    // gif 只给原始帧，帧处置与子矩形合成交给 gif-dispose，它要求索引色输出
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options.read_info(std::io::BufReader::new(file)).ok()?;
    let mut screen = gif_dispose::Screen::new_decoder(&decoder);
    let mut frames = Vec::new();
    let mut delays_ms = Vec::new();
    while let Some(frame) = decoder.read_next_frame().ok()? {
        // GIF 延时以 1/100 秒计；20ms 下限与 Windows 端一致
        let delay = (frame.delay as u32 * 10).max(20);
        screen.blit_frame(frame).ok()?;
        let pixels = screen.pixels_rgba();
        let (w, h) = (pixels.width() as u32, pixels.height() as u32);
        let mut premul_bgra = Vec::with_capacity(w as usize * h as usize * 4);
        for row in pixels.rows() {
            for pixel in row {
                // GIF 只有 1 位 alpha，预乘目前是恒等运算；照做是为了以后缩放仍然正确
                let alpha = pixel.a as u32;
                premul_bgra.push((pixel.b as u32 * alpha / 255) as u8);
                premul_bgra.push((pixel.g as u32 * alpha / 255) as u8);
                premul_bgra.push((pixel.r as u32 * alpha / 255) as u8);
                premul_bgra.push(pixel.a);
            }
        }
        frames.push(Arc::new(Rgba { w, h, premul_bgra }));
        delays_ms.push(delay);
    }
    (!frames.is_empty()).then_some(Clip {
        frames,
        delays_ms,
        loops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 现编一个 2 帧 GIF 再解回来：帧数、延时下限与 BGRA 字节序都得对。
    #[test]
    fn decodes_frames_delays_and_byte_order() {
        let dir = std::env::temp_dir().join("windowsnap-decode-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip.gif");
        {
            let file = std::fs::File::create(&path).unwrap();
            // 调色板：0 = 红，1 = 蓝
            let palette = [255u8, 0, 0, 0, 0, 255];
            let mut encoder = gif::Encoder::new(file, 2, 2, &palette).unwrap();
            for (index, delay) in [(0u8, 7u16), (1u8, 0u16)] {
                let mut frame = gif::Frame::default();
                frame.width = 2;
                frame.height = 2;
                frame.buffer = std::borrow::Cow::Owned(vec![index; 4]);
                frame.delay = delay;
                encoder.write_frame(&frame).unwrap();
            }
        }
        let clip = load_clip(&path, true).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(clip.frames.len(), 2);
        // 7cs → 70ms；0cs 撞到 20ms 下限
        assert_eq!(clip.delays_ms, vec![70, 20]);
        assert_eq!((clip.frames[0].w, clip.frames[0].h), (2, 2));
        assert_eq!(clip.frames[0].premul_bgra.len(), 2 * 2 * 4);
        assert_eq!(&clip.frames[0].premul_bgra[..4], &[0, 0, 255, 255]);
        assert_eq!(&clip.frames[1].premul_bgra[..4], &[255, 0, 0, 255]);
    }
}
