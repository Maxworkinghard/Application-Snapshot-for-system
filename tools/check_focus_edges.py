# -*- coding: utf-8 -*-
import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
for name in ["typing","idle","greeting","snuggle","sleep"]:
    p = os.path.join(base, name + "-focus.mp4")
    cap = cv2.VideoCapture()
    cap.open(p, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    idxs = [0, n//2, n-1]
    got = {}
    for i in idxs:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok:
            continue
        b, g, r = fr[:,:,0].astype(int), fr[:,:,1].astype(int), fr[:,:,2].astype(int)
        ink = (r < 215) | (g < 215) | (b < 215)
        ys, xs = np.where(ink)
        got[i] = (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max()), int(fr.shape[1]), int(fr.shape[0]))
    cap.release()
    for i, v in got.items():
        print(f"{name} frame{i}: bbox x[{v[0]},{v[2]}] y[{v[1]},{v[3]}] of {v[4]}x{v[5]}  l={v[0]} t={v[1]} r={v[4]-1-v[2]} b={v[5]-1-v[3]}")
    if len(got) == 3:
        x0 = min(v[0] for v in got.values()); y0 = min(v[1] for v in got.values())
        x1 = max(v[2] for v in got.values()); y1 = max(v[3] for v in got.values())
        print(f"{name} UNION: x[{x0},{x1}] y[{y0},{y1}] -> need l={x0} t={y0} r={got[0][4]-1-x1} b={got[0][5]-1-y1}")
