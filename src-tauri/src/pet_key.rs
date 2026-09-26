//! 把画进 GIF 里的纯色背景抠成真透明。
//!
//! 有的素材包把白底直接画进了每一帧，桌宠窗口就成了一个白方块。这里按帧处理：
//! 贴着画布边缘的背景连成一片挖掉；被人物圈住的背景（叉腰时胳膊和身体之间的缝）
//! 只有四周不是白色时才挖，人物身上的白（衬衫、领口）留下；边上和背景混在一起的
//! 像素按最近的纯色像素反解回原色，不然深色桌面上会留一圈浅色毛边。处理完重新编成
//! 带透明色的 GIF；本来就是透明背景、或者一帧都没抠掉的，原样返回。

use super::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// 单通道差多少以内仍算背景
const FILL_TOL: f32 = 6.0;
/// 比这还小的封闭背景块当人物身上的细节留下
const HOLE_MIN: usize = 6;
/// 一个通道到多少算白色
const WHITE_MIN_CHANNEL: f32 = 200.0;
/// 封闭块四周的白色少于此比例才当背景挖掉
const HOLE_MAX_WHITE: f64 = 0.25;
/// 离被挖掉的地方这么近的像素算边缘
const BAND: i32 = 2;
/// 人物本身的颜色和背景差不到这个数就不反解，免得把浅色衣服洗花
const MIN_CONTRAST: f32 = 40.0;
/// 反解时最多找这么远，再远就用一层层摊开的近似结果
const SEARCH_LIMIT: i32 = 8;
/// GIF 里留给透明的索引
const TRANSPARENT: u8 = 255;

/// 一个颜色和它在这一帧里出现的次数
type ColorCount = ([u8; 3], usize);

struct Canvas {
    width: usize,
    height: usize,
    rgb: Vec<[f32; 3]>,
    removed: Vec<bool>,
}

/// 四邻域下标，越界的位置是 `usize::MAX`
fn neighbours(index: usize, width: usize, height: usize) -> [usize; 4] {
    let (x, y) = (index % width, index / width);
    [
        if y > 0 { index - width } else { usize::MAX },
        if y + 1 < height {
            index + width
        } else {
            usize::MAX
        },
        if x > 0 { index - 1 } else { usize::MAX },
        if x + 1 < width { index + 1 } else { usize::MAX },
    ]
}

impl Canvas {
    /// 这个像素和背景色的最大单通道差
    fn diff(&self, index: usize, bg: [f32; 3]) -> f32 {
        let pixel = self.rgb[index];
        (0..3).fold(0.0, |worst, channel| {
            worst.max((pixel[channel] - bg[channel]).abs())
        })
    }

    /// 和背景色差得够近，算背景
    fn near(&self, index: usize, bg: [f32; 3]) -> bool {
        self.diff(index, bg) <= FILL_TOL
    }
}

/// 贴着画布边缘的背景连成一片挖掉；人物圈在里面的（胳膊和身体之间的缝）留着后面再看
fn fill_from_border(canvas: &mut Canvas, bg: [f32; 3]) {
    let (width, height) = (canvas.width, canvas.height);
    let mut queue = VecDeque::new();
    for index in 0..canvas.rgb.len() {
        let (x, y) = (index % width, index / width);
        let on_edge = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
        if on_edge && canvas.near(index, bg) {
            canvas.removed[index] = true;
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        for neighbour in neighbours(index, width, height) {
            if neighbour != usize::MAX && !canvas.removed[neighbour] && canvas.near(neighbour, bg) {
                canvas.removed[neighbour] = true;
                queue.push_back(neighbour);
            }
        }
    }
}

/// 被人物圈住的背景再找一遍（上一步只挖了连着画布边缘的）。
///
/// 块四周三到五像素那一圈里白色占多数时留着：那是人物自己的白（衬衫、领口），
/// 挖了就成了窟窿；不然就是真的背景缝，挖掉。
fn enclosed_background(canvas: &mut Canvas, bg: [f32; 3]) {
    let (width, height) = (canvas.width, canvas.height);
    let mut seen = vec![false; canvas.rgb.len()];
    let mut holes = Vec::new();
    for start in 0..canvas.rgb.len() {
        if seen[start] || canvas.removed[start] || !canvas.near(start, bg) {
            continue;
        }
        let mut block = vec![start];
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(index) = queue.pop_front() {
            for neighbour in neighbours(index, width, height) {
                if neighbour != usize::MAX
                    && !seen[neighbour]
                    && !canvas.removed[neighbour]
                    && canvas.near(neighbour, bg)
                {
                    seen[neighbour] = true;
                    block.push(neighbour);
                    queue.push_back(neighbour);
                }
            }
        }
        if block.len() >= HOLE_MIN && whitish_around(&block, canvas) < HOLE_MAX_WHITE {
            holes.push(block);
        }
    }
    // 先看完再一起挖：不然前一块挖掉的地方会影响后一块的四周
    for hole in holes {
        for index in hole {
            canvas.removed[index] = true;
        }
    }
}

/// 这一块四周三到五步（横竖走）能碰到的那一圈里白色的比例。圈上一个像素都没有算 0，
/// 也就是当背景挖掉。
fn whitish_around(block: &[usize], canvas: &Canvas) -> f64 {
    let (width, height) = (canvas.width, canvas.height);
    let mut depth = vec![u8::MAX; canvas.rgb.len()];
    let mut queue = VecDeque::new();
    for &index in block {
        depth[index] = 0;
        queue.push_back(index);
    }
    let (mut whitish, mut total) = (0usize, 0usize);
    while let Some(index) = queue.pop_front() {
        let level = depth[index];
        if level >= 5 {
            continue;
        }
        for neighbour in neighbours(index, width, height) {
            if neighbour == usize::MAX || depth[neighbour] != u8::MAX {
                continue;
            }
            depth[neighbour] = level + 1;
            queue.push_back(neighbour);
            if level + 1 >= 3 && !canvas.removed[neighbour] {
                total += 1;
                if canvas.rgb[neighbour]
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min)
                    >= WHITE_MIN_CHANNEL
                {
                    whitish += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        whitish as f64 / total as f64
    }
}

/// 紧挨着被挖掉的像素这一圈边缘（距离两步以内），和更里面的人物本体
fn edge_bands(canvas: &Canvas) -> (Vec<bool>, Vec<bool>) {
    // 距离 BAND 步以内的偏移都算边缘
    let offsets: Vec<(i32, i32)> = (-BAND..=BAND)
        .flat_map(|dy| (-BAND..=BAND).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| (dx != 0 || dy != 0) && dx * dx + dy * dy <= BAND * BAND)
        .collect();
    let (width, height) = (canvas.width, canvas.height);
    let mut band = vec![false; canvas.rgb.len()];
    let mut inner = vec![false; canvas.rgb.len()];
    for (index, removed) in canvas.removed.iter().copied().enumerate() {
        if removed {
            continue;
        }
        let (x, y) = ((index % width) as i32, (index / width) as i32);
        let touching = offsets.iter().any(|&(dx, dy)| {
            let (nx, ny) = (x + dx, y + dy);
            nx >= 0
                && ny >= 0
                && (nx as usize) < width
                && (ny as usize) < height
                && canvas.removed[ny as usize * width + nx as usize]
        });
        if touching {
            band[index] = true;
        } else {
            inner[index] = true;
        }
    }
    (band, inner)
}

/// 边缘像素和背景混在一起了：按最近的纯色像素反解出没混之前的颜色；解出来一半以上
/// 还是背景的直接挖掉——GIF 的透明只有开和关，不这么做深色桌面上会留一圈浅边。
fn unmix_edges(canvas: &mut Canvas, bg: [f32; 3]) {
    let (band, inner) = edge_bands(canvas);
    if !inner.iter().any(|&inside| inside) {
        return;
    }
    let mut spread = None;
    let mut updates = Vec::new();
    for (index, on_band) in band.iter().copied().enumerate() {
        if !on_band {
            continue;
        }
        let source = match nearest_within(canvas, &inner, index, SEARCH_LIMIT) {
            Some(source) => source,
            None => {
                let nearest = spread.get_or_insert_with(|| nearest_inner(canvas, &inner));
                nearest[index]
            }
        };
        let pixel = canvas.rgb[index];
        let solid = canvas.rgb[source];
        let d = [0, 1, 2].map(|channel| solid[channel] - bg[channel]);
        let norm = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if norm.sqrt() < MIN_CONTRAST {
            continue;
        }
        let alpha = ([0, 1, 2]
            .map(|channel| (pixel[channel] - bg[channel]) * d[channel])
            .iter()
            .sum::<f32>()
            / norm)
            .clamp(0.0, 1.0);
        updates.push((
            index,
            if alpha < 0.5 {
                None
            } else {
                Some([0, 1, 2].map(|channel| {
                    ((pixel[channel] - (1.0 - alpha) * bg[channel]) / alpha).clamp(0.0, 255.0)
                }))
            },
        ));
    }
    // 都按解开之前的值算完了再改，免得后面的边缘拿前面改过的颜色当参考
    for (index, color) in updates {
        match color {
            Some(color) => canvas.rgb[index] = color,
            None => canvas.removed[index] = true,
        }
    }
}

/// 从 `index` 往外一圈圈找最近的「人物本体」像素，超过 `limit` 步就没找到
fn nearest_within(canvas: &Canvas, inner: &[bool], index: usize, limit: i32) -> Option<usize> {
    let (width, height) = (canvas.width as i32, canvas.height as i32);
    let (x, y) = ((index % canvas.width) as i32, (index / canvas.width) as i32);
    let mut best: Option<(i32, usize)> = None;
    for radius in 1..=limit {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= width || ny >= height {
                    continue;
                }
                let candidate = ny as usize * canvas.width + nx as usize;
                if !inner[candidate] {
                    continue;
                }
                let squared = dx * dx + dy * dy;
                if best.is_none_or(|(current, _)| squared < current) {
                    best = Some((squared, candidate));
                }
            }
        }
        if best.is_some_and(|(squared, _)| squared <= radius * radius) {
            break;
        }
    }
    best.map(|(_, index)| index)
}

/// 每个像素最近的人物本体像素，一层层从本体往外摊开（细胳膊细腿那种边缘找不到
/// 本体才用得上，横竖摊开的结果够近）
fn nearest_inner(canvas: &Canvas, inner: &[bool]) -> Vec<usize> {
    let mut source = vec![usize::MAX; canvas.rgb.len()];
    let mut queue = VecDeque::new();
    for (index, &inside) in inner.iter().enumerate() {
        if inside {
            source[index] = index;
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        for neighbour in neighbours(index, canvas.width, canvas.height) {
            if neighbour != usize::MAX && source[neighbour] == usize::MAX {
                source[neighbour] = source[index];
                queue.push_back(neighbour);
            }
        }
    }
    source
}

/// 抠一帧：先挖边缘连通的背景，再处理圈在里面的，最后把边缘混合色反解回原色。
fn key_frame(frame: &RgbaImage, bg: [f32; 3]) -> Canvas {
    let (width, height) = (frame.width() as usize, frame.height() as usize);
    let mut canvas = Canvas {
        width,
        height,
        rgb: frame
            .pixels()
            .map(|pixel| [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32])
            .collect(),
        removed: vec![false; width * height],
    };
    fill_from_border(&mut canvas, bg);
    enclosed_background(&mut canvas, bg);
    unmix_edges(&mut canvas, bg);
    // 边缘反解掉以后，原本被浅色阴影围住的小块背景（鞋跟底下之类）会露出来，再来一轮
    enclosed_background(&mut canvas, bg);
    unmix_edges(&mut canvas, bg);
    canvas
}

/// 把素材里画进去的纯色背景抠掉，编回一份带透明色的 GIF。
///
/// 本来就是透明背景、或者一帧都没抠掉什么的时候返回 None，调用方照原样用。
pub(crate) fn key_out(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let decoded = pet_walk::decode(bytes)?;
    if decoded.is_empty() {
        return Ok(None);
    }
    let delays: Vec<u16> = decoded.iter().map(|(_, delay)| *delay).collect();
    let frames: Vec<RgbaImage> = decoded.into_iter().map(|(image, _)| image).collect();
    let bg = match pet_walk::Backdrop::of(&frames) {
        pet_walk::Backdrop::Transparent => return Ok(None),
        pet_walk::Backdrop::Solid(color) => color,
    };
    let (width, height) = frames[0].dimensions();
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err("画布大得 GIF 存不下".into());
    }
    let background = [bg[0] as f32, bg[1] as f32, bg[2] as f32];
    let mut keyed = Vec::with_capacity(frames.len());
    let mut removed_any = false;
    for frame in &frames {
        let canvas = key_frame(frame, background);
        removed_any |= canvas.removed.iter().any(|&removed| removed);
        keyed.push((frame, canvas));
    }
    if !removed_any {
        return Ok(None);
    }
    encode(&keyed, width, height, &delays).map(Some)
}

/// 编回 GIF：每帧自带一张调色板（颜色要一格不差，不能用有损的量化），
/// 索引 255 留给透明。
fn encode(
    frames: &[(&RgbaImage, Canvas)],
    width: u32,
    height: u32,
    delays: &[u16],
) -> Result<Vec<u8>, String> {
    let (width, height) = (width as u16, height as u16);
    let mut bytes = Vec::new();
    {
        let mut encoder =
            gif::Encoder::new(&mut bytes, width, height, &[]).map_err(|error| error.to_string())?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|error| error.to_string())?;
        for (position, (original, canvas)) in frames.iter().enumerate() {
            let (pixels, palette) = index_frame(original, canvas);
            let mut frame =
                gif::Frame::from_palette_pixels(width, height, pixels, palette, Some(TRANSPARENT));
            frame.delay = delays.get(position).copied().unwrap_or(10);
            // 每帧画完清成透明再画下一帧，不然手脚摆过的地方会留下残影
            frame.dispose = gif::DisposalMethod::Background;
            encoder
                .write_frame(&frame)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(bytes)
}

/// 一帧编成「索引 + 调色板」：抠掉的地方走透明索引，其余按最近的颜色入索引。
fn index_frame(original: &RgbaImage, canvas: &Canvas) -> (Vec<u8>, Vec<u8>) {
    let mut counts = HashMap::<[u8; 3], usize>::new();
    for (index, color) in canvas.rgb.iter().enumerate() {
        if !canvas.removed[index] {
            *counts.entry(rounded(color)).or_default() += 1;
        }
    }
    let palette = palette_colors(&counts, original);
    let mut lookup = HashMap::<[u8; 3], u8>::new();
    let mut pixels = vec![TRANSPARENT; canvas.rgb.len()];
    for (index, color) in canvas.rgb.iter().enumerate() {
        if canvas.removed[index] {
            continue;
        }
        let color = rounded(color);
        let slot = *lookup
            .entry(color)
            .or_insert_with(|| nearest_slot(&palette, color));
        pixels[index] = slot;
    }
    let mut table = Vec::with_capacity(768);
    for color in &palette {
        table.extend_from_slice(color);
    }
    table.resize(768, 0);
    (pixels, table)
}

/// 这一帧的调色板：原素材里本来就有的颜色按出现次数优先占位，剩下的槽位给反解
/// 边缘时新出现的颜色（GIF 一帧最多 255 色，第 256 个索引留给透明）。
fn palette_colors(counts: &HashMap<[u8; 3], usize>, original: &RgbaImage) -> Vec<[u8; 3]> {
    let mut colors: Vec<ColorCount> = counts
        .iter()
        .map(|(&color, &count)| (color, count))
        .collect();
    // 次数相同的按颜色排队，同一份素材每次都编出一样的调色板
    colors.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    if colors.len() <= TRANSPARENT as usize {
        return colors.into_iter().map(|(color, _)| color).collect();
    }
    let known: HashSet<[u8; 3]> = original
        .pixels()
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let (base, extra): (Vec<ColorCount>, Vec<ColorCount>) = colors
        .into_iter()
        .partition(|(color, _)| known.contains(color));
    base.into_iter()
        .chain(extra)
        .map(|(color, _)| color)
        .take(TRANSPARENT as usize)
        .collect()
}

/// 调色板里离这个颜色最近的一格
fn nearest_slot(palette: &[[u8; 3]], color: [u8; 3]) -> u8 {
    let mut best = (u32::MAX, 0);
    for (slot, entry) in palette.iter().enumerate() {
        let distance = (0..3).fold(0u32, |sum, channel| {
            let diff = entry[channel] as i32 - color[channel] as i32;
            sum + (diff * diff) as u32
        });
        if distance < best.0 {
            best = (distance, slot);
        }
    }
    best.1 as u8
}

/// 调色板里用的颜色，四舍五入到 0-255（和离线脚本的 np.rint 一样，.5 归到偶数）
fn rounded(color: &[f32; 3]) -> [u8; 3] {
    [0, 1, 2].map(|channel| color[channel].clamp(0.0, 255.0).round_ties_even() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    const PAPER: Rgba<u8> = Rgba([254, 254, 254, 255]);
    const INK: Rgba<u8> = Rgba([30, 30, 40, 255]);
    const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);

    /// 在 `canvas` 大小的画布上铺一层 `background`，把 `blocks` 里的矩形一块块画上去
    fn painted(
        canvas: (u32, u32),
        background: Rgba<u8>,
        blocks: &[(Rgba<u8>, [u32; 4])],
        frames: usize,
    ) -> Vec<u8> {
        let images: Vec<(RgbaImage, u16)> = (0..frames)
            .map(|_| {
                let mut image = RgbaImage::from_pixel(canvas.0, canvas.1, background);
                for &(color, [left, top, right, bottom]) in blocks {
                    for y in top..bottom {
                        for x in left..right {
                            image.put_pixel(x, y, color);
                        }
                    }
                }
                (image, 10)
            })
            .collect();
        pet_walk::encode(images, canvas.0, canvas.1).expect("生成测试 GIF 失败")
    }

    /// 抠完以后每一帧的样子
    fn keyed_frames(bytes: &[u8]) -> Vec<(RgbaImage, u16)> {
        let keyed = key_out(bytes)
            .expect("抠背景失败")
            .expect("这份素材应当被抠过背景");
        pet_walk::decode(&keyed).expect("抠完的 GIF 解不开")
    }

    #[test]
    fn a_background_painted_into_the_gif_becomes_transparent() {
        let bytes = painted((60, 60), PAPER, &[(INK, [20, 20, 40, 40])], 2);
        let frames = keyed_frames(&bytes);
        assert_eq!(frames.len(), 2);
        for (image, delay) in &frames {
            assert_eq!(*delay, 10, "每帧的时长不能变");
            assert_eq!(image.get_pixel(0, 0)[3], 0, "画布角落的白底应当没了");
            assert_eq!(image.get_pixel(59, 59)[3], 0);
            assert_eq!(*image.get_pixel(30, 30), INK, "人物本体原样保留");
            assert_eq!(*image.get_pixel(20, 20), INK, "平涂的边不用反解");
        }
    }

    #[test]
    fn a_white_part_of_the_figure_keeps_its_colour() {
        // 深色轮廓里套一圈浅色阴影，最里面才是白衬衫：四周不是深色，得当人物自己的白留下
        let grey = Rgba([220, 220, 220, 255]);
        let bytes = painted(
            (80, 80),
            PAPER,
            &[
                (INK, [30, 30, 50, 50]),
                (grey, [33, 33, 47, 47]),
                (PAPER, [36, 36, 44, 44]),
            ],
            1,
        );
        let frames = keyed_frames(&bytes);
        let image = &frames[0].0;
        assert_eq!(*image.get_pixel(40, 40), PAPER, "衬衫应当留着");
        assert_eq!(*image.get_pixel(34, 34), grey, "浅色阴影原样保留");
        assert_eq!(*image.get_pixel(31, 31), INK);
        assert_eq!(image.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn a_gap_between_limbs_is_cut_out() {
        // 深色人物中间的缝是背景色，人物自己不是白，缝得挖掉
        let bytes = painted(
            (80, 80),
            PAPER,
            &[(INK, [20, 20, 60, 60]), (PAPER, [36, 36, 44, 44])],
            1,
        );
        let frames = keyed_frames(&bytes);
        let image = &frames[0].0;
        assert_eq!(image.get_pixel(40, 40)[3], 0, "缝里的背景应当挖掉");
        assert_eq!(*image.get_pixel(25, 25), INK, "人物本体不能被一起挖掉");
    }

    #[test]
    fn an_already_transparent_gif_is_left_alone() {
        let bytes = painted((60, 60), CLEAR, &[(INK, [20, 20, 40, 40])], 1);
        assert!(key_out(&bytes).expect("抠背景失败").is_none());
    }

    #[test]
    fn something_that_is_not_a_gif_is_an_error_not_a_panic() {
        assert!(key_out(b"not a gif").is_err());
    }
}
