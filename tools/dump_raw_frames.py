import cv2, numpy as np, os, sys
sys.stdout.reconfigure(encoding="utf-8")
base = r"C:\Users\32875\Desktop\应用快照\public\media\edge_debug"
os.makedirs(base, exist_ok=True)
def dump(src, tag):
    cap = cv2.VideoCapture(); cap.open(src, cv2.CAP_FFMPEG)
    n = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    for i in [0, n//2]:
        cap.set(cv2.CAP_PROP_POS_FRAMES, i)
        ok, fr = cap.read()
        if not ok: continue
        cv2.imencode(".png", fr)[1].tofile(os.path.join(base, f"{tag}_f{i}.png"))
        hsv = cv2.cvtColor(fr, cv2.COLOR_BGR2HSV)
        cv2.imencode(".png", hsv[:,:,2])[1].tofile(os.path.join(base, f"{tag}_f{i}_V.png"))
        cv2.imencode(".png", hsv[:,:,1])[1].tofile(os.path.join(base, f"{tag}_f{i}_S.png"))
    cap.release()
media = r"C:\Users\32875\Desktop\应用快照\public\media"
dump(os.path.join(media, "打招呼.mp4"), "greeting_raw")
dump(os.path.join(media, "撒娇.mp4"), "snuggle_raw")
dump(os.path.join(media, "睡觉.mp4"), "sleep_raw")
dump(os.path.join(media, "敲键盘.mp4"), "typing_raw")
print("dumped")
