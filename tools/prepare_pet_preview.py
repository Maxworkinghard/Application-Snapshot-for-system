from __future__ import annotations

import subprocess
from pathlib import Path

import cv2
import numpy as np
from rembg import new_session, remove


ROOT = Path(__file__).resolve().parents[1]
MEDIA = ROOT / "public" / "media"
FFMPEG = Path(r"C:\Users\32875\AppData\Roaming\TRAE SOLO CN\ModularData\ai-agent\vm\tools\app\ffmpeg\ffmpeg.exe")
SESSION = new_session("u2net")


def clean_alpha_islands(rgba: np.ndarray) -> np.ndarray:
    """Drop tiny disconnected alpha specks left by the source footage."""
    alpha = rgba[:, :, 3]
    binary = (alpha > 8).astype(np.uint8)
    count, labels, stats, _ = cv2.connectedComponentsWithStats(binary, connectivity=8)
    if count <= 2:
        return rgba
    areas = stats[1:, cv2.CC_STAT_AREA]
    main_label = int(np.argmax(areas)) + 1
    main_area = areas[main_label - 1]
    keep = np.zeros(count, dtype=bool)
    keep[0] = True
    keep[main_label] = True
    for label in range(1, count):
        if label == main_label:
            continue
        if areas[label - 1] >= max(600.0, main_area * 0.004):
            keep[label] = True
    rgba[:, :, 3] = np.where(keep[labels], alpha, 0)
    return rgba


def connected_background(frame: np.ndarray) -> np.ndarray:
    """Find background pixels reachable from the frame edge.

    The cat contains dark fur, so a global black key would destroy its detail.
    Fur is protected by a texture/saturation barrier, so the flood can only
    travel through smooth background, including gradual shadows.
    """
    height, width = frame.shape[:2]
    grey = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
    saturation = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)[:, :, 1]
    texture = np.abs(cv2.Laplacian(grey, cv2.CV_32F, ksize=3))
    texture = cv2.boxFilter(texture, -1, (5, 5))

    # Dark fur is low saturation, but it is strongly textured. The wedge-like
    # shadows around the keyboard are smooth, so this barrier keeps fur while
    # letting the flood pass through background gradients.
    barrier = (saturation > 60) | ((grey < 120) & (texture > 7.0))
    work = frame.copy()
    work[barrier] = (255, 0, 0)

    flood_mask = np.zeros((height + 2, width + 2), np.uint8)
    step = max(8, min(height, width) // 60)
    edge_points = []
    edge_points.extend((x, 0) for x in range(0, width, step))
    edge_points.extend((x, height - 1) for x in range(0, width, step))
    edge_points.extend((0, y) for y in range(0, height, step))
    edge_points.extend((width - 1, y) for y in range(0, height, step))

    for x, y in edge_points:
        if flood_mask[y + 1, x + 1] or work[y, x, 0] == 255:
            continue
        cv2.floodFill(
            work,
            flood_mask,
            (x, y),
            0,
            (14, 14, 14),
            (14, 14, 14),
            cv2.FLOODFILL_MASK_ONLY | (255 << 8) | 4,
        )
    return flood_mask[1:-1, 1:-1] > 0


def process_video(source: Path) -> None:
    capture = cv2.VideoCapture(str(source))
    if not capture.isOpened():
        raise RuntimeError(f"cannot open {source}")

    width = int(capture.get(cv2.CAP_PROP_FRAME_WIDTH))
    height = int(capture.get(cv2.CAP_PROP_FRAME_HEIGHT))
    fps = capture.get(cv2.CAP_PROP_FPS) or 24.0
    output = MEDIA / f"{source.stem}-stage.mp4"
    command = [
        str(FFMPEG), "-y", "-f", "rawvideo", "-vcodec", "rawvideo",
        "-pix_fmt", "bgr24", "-s", f"{width}x{height}", "-r", str(fps),
        "-i", "-", "-an", "-c:v", "libx264", "-preset", "medium",
        "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart",
        str(output),
    ]
    encoder = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)

    try:
        while True:
            ok, frame = capture.read()
            if not ok:
                break
            ok_png, png = cv2.imencode(".png", frame)
            if not ok_png:
                raise RuntimeError("cannot encode frame")
            cut = remove(png.tobytes(), session=SESSION)
            rgba = cv2.imdecode(np.frombuffer(cut, np.uint8), cv2.IMREAD_UNCHANGED)
            rgba = clean_alpha_islands(rgba)
            # The typing clip has a soft smoke puff near the tail that the
            # matting model keeps as a translucent ghost. Remove weak alpha
            # away from the strongest subject area (the cat and keyboard).
            if source.stem == "typing":
                alpha_channel = rgba[:, :, 3]
                weak = (alpha_channel > 0) & (alpha_channel < 170)
                if weak.any():
                    near_subject = cv2.dilate(
                        (alpha_channel >= 200).astype(np.uint8),
                        np.ones((31, 31), np.uint8),
                    ).astype(bool)
                    rgba[:, :, 3] = np.where(weak & ~near_subject, 0, alpha_channel)
            alpha = rgba[:, :, 3:4].astype(np.float32) / 255.0
            white = np.full_like(rgba[:, :, :3], 255)
            frame = (white * (1 - alpha) + rgba[:, :, :3] * alpha).astype(np.uint8)
            encoder.stdin.write(frame.tobytes())
    finally:
        capture.release()
        if encoder.stdin:
            encoder.stdin.close()
        stderr = encoder.stderr.read().decode("utf-8", errors="replace")
        result = encoder.wait()
        if result != 0:
            raise RuntimeError(f"ffmpeg failed for {source}: {stderr[-2000:]}")
    print(f"created {output.name} ({width}x{height} @ {fps:g}fps, rembg)")


if __name__ == "__main__":
    import sys

    names = sys.argv[1:] or ("typing.mp4", "idle.mp4", "greeting.mp4", "snuggle.mp4", "sleep.mp4")
    for name in names:
        process_video(MEDIA / name)
