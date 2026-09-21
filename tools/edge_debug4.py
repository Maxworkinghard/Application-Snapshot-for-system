import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
def analyze(name):
    p = os.path.join(base, name + "-focus.mp4")
    cap = cv2.VideoCapture(); cap.open(p, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    hs = []; ss = []; vs = []
    for i in [0, n//2]:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok: continue
        hsv = cv2.cvtColor(fr, cv2.COLOR_BGR2HSV)
        hs.append(hsv[:,:,0]); ss.append(hsv[:,:,1]); vs.append(hsv[:,:,2])
    cap.release()
    if not hs: return
    H = np.median(np.stack(hs,0),0); S = np.median(np.stack(ss,0),0); V = np.median(np.stack(vs,0),0)
    for thr in [60, 80, 100, 120, 150, 180, 200, 215, 230, 245]:
        white = (V >= thr) & (S <= 45)
        wfrac = white.mean()
        ys, xs = np.where(~white)
        print(f"{name} thr{thr}: whiteFrac={wfrac:.3f} bbox x[{xs.min()},{xs.max()}] y[{ys.min()},{ys.max()}]")
for nm in ["typing","greeting","snuggle","sleep"]:
    analyze(nm)
