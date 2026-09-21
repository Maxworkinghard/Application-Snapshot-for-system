import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
def ink_mask(fr):
    hsv = cv2.cvtColor(fr, cv2.COLOR_BGR2HSV)
    H, S, V = hsv[:,:,0], hsv[:,:,1], hsv[:,:,2]
    b, g, r = fr[:,:,0].astype(int), fr[:,:,1].astype(int), fr[:,:,2].astype(int)
    mx = np.maximum(np.maximum(r,g),b); mn = np.minimum(np.minimum(r,g),b)
    sat = mx - mn; lum = (r+g+b)/3.0
    colorful = (sat > 45)
    offwhite = (S <= 45) & (V < 232)
    dark = (V <= 35)
    return (colorful | offwhite | dark).astype(np.uint8)
def biggest_component(mask):
    n, lab, stats, cent = cv2.connectedComponentsWithStats(mask, 8)
    if n <= 1: return mask
    idx = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
    return (lab == idx).astype(np.uint8)
def process(src_name, dst_name):
    src = os.path.join(base, src_name)
    dst = os.path.join(base, dst_name)
    cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
    if not cap.isOpened():
        print("open fail", src_name); return
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    fps = cap.get(cv2.CAP_PROP_FPS) or 30
    idxs = np.linspace(0, max(n-1,0), min(40, n)).astype(int)
    sample = []
    for i in idxs:
        cap.set(cv2.CAP_PROP_POS_FRAMES, int(i))
        ok, fr = cap.read()
        if ok: sample.append(fr)
    cap.release()
    if not sample:
        print("no frames", src_name); return
    h0, w0 = sample[0].shape[:2]
    ink_acc = np.zeros((h0, w0), np.uint8)
    for fr in sample:
        m = ink_mask(fr)
        m = biggest_component(m)
        ink_acc = cv2.bitwise_or(ink_acc, m)
    ink_acc = cv2.dilate(ink_acc, np.ones((7,7), np.uint8))
    ys, xs = np.where(ink_acc > 0)
    x0, x1, y0, y1 = int(xs.min()), int(xs.max()), int(ys.min()), int(ys.max())
    print(f"{src_name}: crop x[{x0},{x1}] y[{y0},{y1}] of {w0}x{h0}")
    side = max(x1-x0+1, y1-y0+1)
    cx, cy = (x0+x1)//2, (y0+y1)//2
    half = side//2
    l = max(cx-half, 0); t = max(cy-half, 0)
    r = min(l+side, w0); b_ = min(t+side, h0)
    l = max(r-side, 0); t = max(b_-side, 0)
    if r-l != b_-t:
        side2 = min(r-l, b_-t)
        l = max(0, l); t = max(0, t); r = l+side2; b_ = t+side2
    print(f"  square -> final {r-l}x{b_-t}")
    writer = cv2.VideoWriter()
    fourcc = cv2.VideoWriter_fourcc(*"mp4v")
    side_final = r-l
    tmp = dst + ".tmp.mp4"
    okw = writer.open(tmp, fourcc, fps, (side_final, side_final))
    if not okw:
        print("writer open fail"); return
    cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
    while True:
        ok, fr = cap.read()
        if not ok: break
        crop = fr[t:b_, l:r]
        if crop.shape[0] != side_final or crop.shape[1] != side_final:
            crop = cv2.resize(crop, (side_final, side_final))
        writer.write(crop)
    cap.release(); writer.release()
    os.replace(tmp, dst)
    print("  wrote", dst_name, side_final)
pairs = [
    ("typing.mp4", "typing-clean.mp4"),
    ("idle.mp4", "idle-clean.mp4"),
    ("打招呼.mp4", "greeting-clean.mp4"),
    ("撒娇.mp4", "snuggle-clean.mp4"),
    ("睡觉.mp4", "sleep-clean.mp4"),
]
for s, d in pairs:
    process(s, d)
