//! 从进程可执行文件路径猜测桌面图标（best-effort）。

use image::{Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 尝试为 pid 找一张不超过 `size` 像素边长的图标。失败返回 None，调用方显示占位即可。
pub fn icon_for_process(pid: u32, size: u32) -> Option<RgbaImage> {
    let exe = fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let name = exe.file_name()?.to_string_lossy().to_string();
    // 去掉常见后缀
    let stem = name
        .trim_end_matches("-bin")
        .trim_end_matches(".bin")
        .to_string();

    let icon_name = find_desktop_icon(&stem).unwrap_or(stem);
    load_icon_file(&icon_name)
        .or_else(|| load_named_icon(&icon_name, size))
        .map(|image| fit(image, size))
}

/// 比要的大就等比缩下来：窗口列表只显示 24px，没必要把 256px 的原图传给页面。
fn fit(image: RgbaImage, size: u32) -> RgbaImage {
    if image.width() <= size && image.height() <= size {
        return image;
    }
    image::DynamicImage::ImageRgba8(image)
        .resize(size, size, image::imageops::FilterType::Triangle)
        .into_rgba8()
}

/// hicolor 主题的尺寸目录，按「刚好够用」排序：先由小到大取不小于 `size` 的，
/// 都没有再由大到小取更小的。这样既不糊，也不白白读一张大图再缩。
fn size_dirs_for(size: u32) -> Vec<&'static str> {
    const AVAILABLE: [(u32, &str); 5] = [
        (32, "32x32"),
        (48, "48x48"),
        (64, "64x64"),
        (128, "128x128"),
        (256, "256x256"),
    ];
    let mut dirs: Vec<&str> = AVAILABLE
        .iter()
        .filter(|(edge, _)| *edge >= size)
        .map(|(_, dir)| *dir)
        .collect();
    dirs.extend(
        AVAILABLE
            .iter()
            .rev()
            .filter(|(edge, _)| *edge < size)
            .map(|(_, dir)| *dir),
    );
    dirs
}

fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home));
    } else if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share"));
    }
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for part in data_dirs.split(':').filter(|s| !s.is_empty()) {
            dirs.push(PathBuf::from(part));
        }
    } else {
        dirs.push(PathBuf::from("/usr/local/share"));
        dirs.push(PathBuf::from("/usr/share"));
    }
    dirs
}

fn find_desktop_icon(app: &str) -> Option<String> {
    let lower = app.to_ascii_lowercase();
    for root in xdg_data_dirs() {
        let apps = root.join("applications");
        let Ok(entries) = fs::read_dir(&apps) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let matches_name = file_stem.eq_ignore_ascii_case(app)
                || file_stem.to_ascii_lowercase().contains(&lower);
            let matches_exec = text.lines().any(|line| {
                let line = line.trim();
                line.starts_with("Exec=") && line.to_ascii_lowercase().contains(&lower)
            });
            if !(matches_name || matches_exec) {
                continue;
            }
            for line in text.lines() {
                let line = line.trim();
                if let Some(icon) = line.strip_prefix("Icon=") {
                    let icon = icon.trim();
                    if !icon.is_empty() {
                        return Some(icon.to_string());
                    }
                }
            }
        }
    }
    None
}

fn load_icon_file(icon: &str) -> Option<RgbaImage> {
    let path = Path::new(icon);
    if path.is_absolute() && path.is_file() {
        return image::open(path).ok().map(|img| img.into_rgba8());
    }
    None
}

fn load_named_icon(name: &str, edge: u32) -> Option<RgbaImage> {
    // 直接搜 hicolor，避免链 GTK。
    let exts = ["png", "svg"];
    for root in xdg_data_dirs() {
        for size in size_dirs_for(edge) {
            for ext in exts {
                let candidate = root
                    .join("icons/hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    if ext == "svg" {
                        if let Some(png) = rasterize_svg(&candidate, edge) {
                            return Some(png);
                        }
                    } else if let Ok(img) = image::open(&candidate) {
                        return Some(img.into_rgba8());
                    }
                }
            }
        }
        // pixmaps 兜底
        for ext in exts {
            let candidate = root.join("pixmaps").join(format!("{name}.{ext}"));
            if candidate.is_file() {
                if let Ok(img) = image::open(&candidate) {
                    return Some(img.into_rgba8());
                }
            }
        }
    }
    None
}

fn rasterize_svg(path: &Path, edge: u32) -> Option<RgbaImage> {
    // 可选：rsvg-convert；没有就跳过 SVG
    let edge = edge.to_string();
    let output = Command::new("rsvg-convert")
        .args(["-w", &edge, "-h", &edge])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    image::load_from_memory(&output.stdout)
        .ok()
        .map(|img| img.into_rgba8())
        .or_else(|| {
            // 有些环境输出到文件更稳，这里内存失败就放弃
            let _ = Rgba([0, 0, 0, 0]);
            None
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_smallest_size_that_is_big_enough_first() {
        assert_eq!(size_dirs_for(48)[0], "48x48");
        assert_eq!(size_dirs_for(50)[0], "64x64");
        assert_eq!(
            size_dirs_for(300),
            vec!["256x256", "128x128", "64x64", "48x48", "32x32"]
        );
        assert_eq!(
            size_dirs_for(20),
            vec!["32x32", "48x48", "64x64", "128x128", "256x256"]
        );
    }

    #[test]
    fn fit_shrinks_but_never_enlarges() {
        assert_eq!(fit(RgbaImage::new(256, 256), 64).dimensions(), (64, 64));
        assert_eq!(fit(RgbaImage::new(32, 32), 64).dimensions(), (32, 32));
    }
}
