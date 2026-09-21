import cv2, numpy as np
from pathlib import Path
MEDIA = Path(r'C:\Users\32875\Desktop\应用快照\public\media')
FFMPEG = r'C:\Users\32875\AppData\Roaming\TRAE SOLO CN\ModularData\ai-agent\vm\tools\app\ffmpeg\ffmpeg.exe'
names = ['typing','idle','greeting','snuggle','sleep']
THR = 243
MAXW = 640; MAXH = 640; PAD = 18
for n in names:
    src = MEDIA / (n + '-stage.mp4')
    cap = cv2.VideoCapture(str(src))
    W = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)); H = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
    N = int(cap.get(cv2.CAP_PROP_FRAME_COUNT)) or 0
    fps = cap.get(cv2.CAP_PROP_FPS) or 24.0
    frames = []
    for i in range(N):
        ok, f = cap.read()
        if not ok: break
        frames.append(f)
    cap.release()
    if not frames:
        print(n, 'NO FRAMES'); continue
    x0, y0, x1, y1 = W, H, 0, 0
    for f in frames:
        b,g,r = f[:,:,0].astype(int), f[:,:,1].astype(int), f[:,:,2].astype(int)
        ink = ((r<THR)|(g<THR)|(b<THR)).astype(np.uint8)
        ink = cv2.morphologyEx(ink, cv2.MORPH_OPEN, np.ones((3,3), np.uint8))
        ys, xs = np.where(ink>0)
        if len(ys)==0: continue
        x0=min(x0,int(xs.min())); y0=min(y0,int(ys.min())); x1=max(x1,int(xs.max())+1); y1=max(y1,int(ys.max())+1)
    x0=max(0,x0-PAD); y0=max(0,y0-PAD); x1=min(W,x1+PAD); y1=min(H,y1+PAD)
    iw, ih = x1-x0, y1-y0
    k = min(1.0, MAXW/iw, MAXH/ih)
    nw, nh = max(2, int(round(iw*k))//2*2), max(2, int(round(ih*k))//2*2)
    out = MEDIA / (n + '-focus.mp4')
    vf = f'crop={iw}:{ih}:{x0}:{y0},scale={nw}:{nh}:flags=lanczos'
    import subprocess
    subprocess.run([FFMPEG,'-y','-i',str(src),'-vf',vf,'-an','-c:v','libx264','-preset','medium','-crf','18','-pix_fmt','yuv420p','-movflags','+faststart',str(out)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(n, 'src', str(W)+'x'+str(H), 'crop', f'{x0},{y0} {iw}x{ih}', '->', str(nw)+'x'+str(nh))

