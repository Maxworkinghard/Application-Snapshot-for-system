import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
os.makedirs(os.path.join(base, "edge_debug"), exist_ok=True)
src = os.path.join(base, "typing.mp4")
cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT)); fps = cap.get(cv2.CAP_PROP_FPS) or 30
w0 = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)); h0 = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
print("typing size", w0, h0, "frames", n)
# right-top cell approx
l, t, r, b = int(w0*0.60), int(h0*0.04), int(w0*0.98), int(h0*0.46)
print("cell", l, t, r, b)
writer = cv2.VideoWriter(); fourcc = cv2.VideoWriter_fourcc(*"mp4v")
tmp = os.path.join(base, "typing-single.tmp.mp4")
cw, chh = r-l, b-t
writer.open(tmp, fourcc, fps, (cw, chh))
cap.release()
cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
while True:
    ok, fr = cap.read()
    if not ok: break
    crop = fr[t:b, l:r]
    writer.write(crop)
cap.release(); writer.release()
os.replace(tmp, os.path.join(base, "typing-single.mp4"))
print("wrote typing-single.mp4", cw, chh)
cap = cv2.VideoCapture(); cap.open(os.path.join(base, "typing-single.mp4"), cv2.CAP_FFMPEG)
cap.set(cv2.CAP_PROP_POS_FRAMES, 0); ok, fr = cap.read(); cap.release()
if ok:
    cv2.imencode(".png", fr)[1].tofile(os.path.join(base, "edge_debug", "typing_single_first.png"))
