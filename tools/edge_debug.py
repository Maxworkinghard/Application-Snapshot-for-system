import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
for name in ["typing","snuggle","sleep"]:
    p = os.path.join(base, name + "-focus.mp4")
    cap = cv2.VideoCapture()
    cap.open(p, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    rows = []
    for i in [0, n//2, n-1]:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok: continue
        b, g, r = fr[:,:,0].astype(int), fr[:,:,1].astype(int), fr[:,:,2].astype(int)
        ink = ((r < 215) | (g < 215) | (b < 215)).astype(np.uint8)
        h, w = ink.shape
        top = ink[:4,:].mean(); bot = ink[h-4:,:].mean(); left = ink[:,:4].mean(); right = ink[:,w-4:].mean()
        rows.append((i, top, bot, left, right))
    cap.release()
    for i, t, bo, l, rr in rows:
        print(f"{name} frame{i}: inkTop={t:.3f} inkBot={bo:.3f} inkLeft={l:.3f} inkRight={rr:.3f}")
    cap = cv2.VideoCapture(); cap.open(p, cv2.CAP_FFMPEG)
    cap.set(cv2.CAP_PROP_POS_FRAMES, 0); ok, fr = cap.read(); cap.release()
    if ok:
        cv2.imencode(".png", fr)[1].tofile(os.path.join(base, "edge_debug", f"{name}_frame0.png"))
