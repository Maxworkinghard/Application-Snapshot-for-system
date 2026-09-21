import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
def analyze(name):
    p = os.path.join(base, name + "-focus.mp4")
    cap = cv2.VideoCapture(); cap.open(p, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    hsv_frames = []
    frames = []
    for i in [0, n//2]:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok: continue
        hsv = cv2.cvtColor(fr, cv2.COLOR_BGR2HSV)
        hsv_frames.append(hsv); frames.append(fr)
    cap.release()
    if not hsv_frames: return
    H = np.median(np.stack([f[:,:,0] for f in hsv_frames], 0), 0)
    S = np.median(np.stack([f[:,:,1] for f in hsv_frames], 0), 0)
    V = np.median(np.stack([f[:,:,2] for f in hsv_frames], 0), 0)
    h0 = frames[0].shape[0]; w0 = frames[0].shape[1]
    # colorful: saturated
    colorful = (S > 45).astype(np.uint8)
    # fur: low saturation, mid value
    fur = ((S <= 45) & (V < 215) & (V > 35)).astype(np.uint8)
    dark = (V <= 35).astype(np.uint8)
    ink = colorful | fur | dark
    ys, xs = np.where(ink)
    print(f"{name}: mask bbox x[{xs.min()},{xs.max()}] y[{ys.min()},{ys.max()}] of {w0}x{h0}")
    dbg = frames[0].copy()
    dbg[ink.astype(bool)] = (0, 0, 255)
    cv2.imencode(".png", dbg)[1].tofile(os.path.join(base, "edge_debug", f"{name}_mask.png"))
for nm in ["typing","idle","greeting","snuggle","sleep"]:
    analyze(nm)
