import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
def analyze(name):
    p = os.path.join(base, name + "-focus.mp4")
    cap = cv2.VideoCapture(); cap.open(p, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    for i in [0, n//2]:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok: continue
        b, g, r = fr[:,:,0].astype(int), fr[:,:,1].astype(int), fr[:,:,2].astype(int)
        mx = np.maximum(np.maximum(r, g), b)
        mn = np.minimum(np.minimum(r, g), b)
        sat = mx - mn
        lum = (r + g + b) / 3.0
        ink = ((r < 215) | (g < 215) | (b < 215)).astype(np.uint8)
        strong = ((lum < 120) & (sat > 30)).astype(np.uint8)
        ys, xs = np.where(strong)
        print(f"{name} frame{i}: strong bbox x[{xs.min()},{xs.max()}] y[{ys.min()},{ys.max()}] of {fr.shape[1]}x{fr.shape[0]}")
        ys2, xs2 = np.where(ink)
        print(f"{name} frame{i}: ink     bbox x[{xs2.min()},{xs2.max()}] y[{ys2.min()},{ys2.max()}]")
        dbg = fr.copy()
        dbg[strong.astype(bool)] = (0, 0, 255)
        cv2.imencode(".png", dbg)[1].tofile(os.path.join(base, "edge_debug", f"{name}_f{i}_strong.png"))
    cap.release()
for nm in ["typing","idle","greeting","snuggle","sleep"]:
    analyze(nm)
