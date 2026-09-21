import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media"
src = os.path.join(base, "typing-single.mp4")
dst = os.path.join(base, "typing-stage.mp4")
cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT)); fps = cap.get(cv2.CAP_PROP_FPS) or 30
h0 = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)); w0 = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
tb = 60
writer = cv2.VideoWriter(); fourcc = cv2.VideoWriter_fourcc(*"mp4v")
tmp = dst + ".tmp.mp4"
writer.open(tmp, fourcc, fps, (w0, h0 - tb))
cap.release()
cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
while True:
    ok, fr = cap.read()
    if not ok: break
    writer.write(fr[:h0-tb, :])
cap.release(); writer.release()
os.replace(tmp, dst)
print("rewrote typing-stage.mp4", w0, h0 - tb)
