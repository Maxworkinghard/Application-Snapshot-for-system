import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
from rembg import remove, new_session
sess = new_session("u2net")
def process(src_name, dst_name):
    src = os.path.join(base, src_name)
    dst = os.path.join(base, dst_name)
    cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
    if not cap.isOpened():
        print("open fail", src_name); return
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    fps = cap.get(cv2.CAP_PROP_FPS) or 30
    w0 = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)); h0 = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    cap.release()
    side = 720 if max(w0, h0) >= 720 else max(w0, h0)
    tmp = dst + ".tmp.mp4"
    writer = cv2.VideoWriter()
    fourcc = cv2.VideoWriter_fourcc(*"mp4v")
    if not writer.open(tmp, fourcc, fps, (side, side)):
        print("writer fail", dst_name); return
    cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
    fi = 0
    while True:
        ok, fr = cap.read()
        if not ok: break
        rgb = cv2.cvtColor(fr, cv2.COLOR_BGR2RGB)
        out = remove(rgb, session=sess)
        a = out[:,:,3]
        ys, xs = np.where(a > 8)
        if len(xs) == 0:
            writer.write(np.zeros((side, side, 3), np.uint8)); fi += 1; continue
        x0, x1, y0, y1 = int(xs.min()), int(xs.max()), int(ys.min()), int(ys.max())
        crop = out[y0:y1+1, x0:x1+1]
        ch, cw = crop.shape[:2]
        scale = (side * 0.92) / max(ch, cw)
        nw, nh = max(1, int(round(cw * scale))), max(1, int(round(ch * scale)))
        crop = cv2.resize(crop, (nw, nh), interpolation=cv2.INTER_AREA)
        canvas = np.zeros((side, side, 4), np.uint8)
        ox = (side - nw) // 2; oy = (side - nh) // 2
        canvas[oy:oy+nh, ox:ox+nw] = crop
        b, g, r_, a_ = canvas[:,:,0], canvas[:,:,1], canvas[:,:,2], canvas[:,:,3]
        alpha = (a_.astype(np.float32) / 255.0)[:,:,None]
        white = np.ones_like(canvas[:,:,:3], np.float32) * 255
        comp = (canvas[:,:,:3].astype(np.float32) * alpha + white * (1 - alpha)).astype(np.uint8)
        bgr = cv2.cvtColor(comp, cv2.COLOR_RGB2BGR)
        writer.write(bgr)
        fi += 1
        if fi == 1:
            cv2.imencode(".png", bgr)[1].tofile(os.path.join(base, "edge_debug", dst_name.replace(".mp4", "_first.png")))
    cap.release(); writer.release()
    os.replace(tmp, dst)
    print("wrote", dst_name, "frames", fi)
pairs = [
    ("typing.mp4", "typing-clean.mp4"),
    ("idle.mp4", "idle-clean.mp4"),
    ("打招呼.mp4", "greeting-clean.mp4"),
    ("撒娇.mp4", "snuggle-clean.mp4"),
    ("睡觉.mp4", "sleep-clean.mp4"),
]
for s, d in pairs:
    process(s, d)
