# Linux parity for Tauri mainline

## Summary

Bring the **repository-root Tauri 2 app** on Linux closer to the Windows-polished experience. `linux/windowsnap` remains reference-only (still what GitHub Release packs today); this PR does not invent a second product.

## Branch / commits

- Branch: `adapt/linux-parity` (local only; not pushed)
- See `git log adapt/linux-parity --oneline` for the full tip chain.

## What was behind (gap inventory)

- **Docs / packaging**: Linux users were pointed at `linux/windowsnap` and generic `npm run tauri` without Debian/Ubuntu dependency lists, Wayland caveats, or an opt-in autostart story matching Settings.
- **Recording**: ffmpeg `x11grab` hard-coded `:0.0`, so `DISPLAY=:3` / remote / multi-seat sessions recorded the wrong screen; pure Wayland had no clear error / no portal path.
- **Minimized windows**: non-Windows path always failed with “请先还原”; Windows restores via `ShowWindow`.
- **App icons**: previous-app / window list icons were Windows-only (`None` elsewhere).
- **Autostart**: Settings stored `launchOnBoot` but never wrote an OS hook on any platform; Linux can do XDG autostart opt-in.
- **Tray**: tray build failure aborted `setup` (bad for sessions without StatusNotifierHost).
- **Recording save dir**: UI `saveDir` was ignored; always used Downloads.
- **Include cursor**: preference stored but not applied to Linux `x11grab`.
- **Release notes**: Linux install instructions only covered the reference `windowsnap` tarball.
- **Prefs that showed in UI but did nothing**: `clipboardAutoClear`, `snapshotFormat`, `autoSaveLocal`, history `saveDir`, `afterCapture`, flash/shutter hooks. (`trayDoubleClick` removed entirely — see tray note.)
- **Capture modes**: region / fullscreen / scrolling were stub strings only.

## What changed

### Runtime (Tauri `src-tauri`)

- New `src-tauri/src/linux/` adapters: recording (`$DISPLAY` x11grab, Wayland portal ScreenCast via xcap + PipeWire frames → ffmpeg `rawvideo`, cursor flag on x11grab), `xdotool` restore, XDG autostart, best-effort Freedesktop icons, optional ffmpeg still frame with cursor, **scrolling long-capture** (X11 window frames + xdotool Page_Down/wheel + vertical overlap stitch).
- Soft-fail system tray on Linux so missing StatusNotifierHost does not kill startup.
- Honor `saveDir` (with `~/` expansion) for recording **and** snapshot history files; `platform_capabilities` command for diagnostics.
- Frontend types + `platformCapabilities()` API wrapper.
- Linux-only `zbus` dep for ScreenCast portal probing (session bus introspect).
- **Preference wiring**: `clipboard_auto_clear` (30s/60s/5m/never), `snapshot_format` (png/jpeg/webp), `auto_save_local`, history `save_dir`, `after_capture` (`clipboard` / `saveas` / **`annotate`**), `flash_on_capture` / shutter sound via `capture-feedback` event, `hide_after_copy`.
- **Capture modes**: `fullscreen` (xcap primary monitor; Linux+`include_cursor` may use ffmpeg x11grab one frame); `region` (optional `slop`, else fullscreen overlay picker → xcap `capture_region`); **`scrolling`** X11 best-effort (Wayland returns bilingual error).
- **Annotate after-capture**: dedicated `annotate` webview (pen + rect, copy / save / close); no longer coerced to clipboard.
- **Still-image cursor**: Linux `include_cursor` uses ffmpeg x11grab for **fullscreen, window, and region** rects (fallback to xcap without cursor).
- **Tray activation**: tray double-click preference **removed**. Left-click (and Windows double-click) always opens the main window where the platform emits tray clicks; right-click menu remains 打开设置 / 退出. Linux may still be menu-only (see below).

### Docs / scripts

- `scripts/linux/{install-deps,check-env,build}.sh` + README for Tauri mainline.
- **Optional** systemd `--user` unit example: `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example` (advanced; Settings XDG remains the supported opt-in; never enabled by the app).
- Root `README.md` / `README.zh-CN.md` Linux sections; `linux/README.md` reference banner; release-notes clarify tarball vs Tauri.
- `tauri.conf.json` Linux deb runtime depends + `region-picker` + `annotate` windows.

## Maintainer decisions (locked in)

1. **Autostart:** Settings → XDG desktop autostart is the primary opt-in. Optional systemd user unit is **documented only** (not default, does not replace XDG).
2. **Recording:** Portal-based Wayland recording (xdg-desktop-portal ScreenCast / PipeWire) is implemented as the pure-Wayland path; X11/`$DISPLAY` x11grab remains preferred when available.
3. **Release:** Leave Release packaging as-is (`windowsnap` tarball). No CI artifact switch in this PR.
4. **Honesty over fake settings:** tray double-click preference removed (no fake Linux UI). Annotate and scrolling are wired for real.

## Tray clicks (preference removed)

User decision: the 「双击托盘图标」 preference is unnecessary. Single-click should open the main app when the platform supports tray clicks.

- **Win/macOS:** left-click (and Windows double-click) → `show_main_window`. Menu: 打开设置 / 退出.
- **Linux** (`tauri` → `tray-icon 0.24` / libayatana-appindicator): still **no Activate / left-click callback**; menu works when a StatusNotifierHost is present. `trayNote` documents this. `linux-ksni` not in the pinned tray-icon release.

## Test plan / results (this box)

- `cargo test --lib` — **21 passed** (prefs helpers + portal preference / env tests + scrolling stitch unit tests)
- `npm run build` (tsc + vite) — **pass**
- `bash scripts/linux/check-env.sh` — DISPLAY / portal / PipeWire probes (portal may be “not visible” on agent desktop without a full session)
- ffmpeg smoke: `x11grab` against `$DISPLAY` previously verified
- **Not run / needs real session:** interactive annotate UX, region-picker / slop UX, scrolling against a live scrollable window, portal Screenshot chooser E2E, tray click on Windows/macOS, full `npm run tauri build` bundlers (deb/AppImage), interactive global shortcuts, keyring Secret Service end-to-end

## Remaining / verification limits

- Portal path **feature-detects** and returns bilingual errors when session D-Bus / ScreenCast / PipeWire are missing; full E2E needs a compositor that shows the portal chooser.
- Portal recording uses xcap’s ScreenCast helper: source is **user-picked** (monitor-oriented); it does not map the in-app “previous window” id. `includeCursor` on the portal path follows compositor/portal defaults (cursor_mode not exposed by xcap’s helper yet).
- **Still-image cursor:** applied on Linux fullscreen / window / region via ffmpeg x11grab when `include_cursor` is on and DISPLAY exists; falls back to xcap without cursor.
- **Scrolling:** X11/`$DISPLAY` + xdotool only; pure Wayland returns a clear bilingual error. Overlap stitch is best-effort (max ~28 frames); odd scrollable widgets may stop early or produce imperfect seams.
- **Minimized restore:** X11 via xdotool; Wayland returns a clear bilingual “manual restore” error (no portable portal for activate-window).
- Windows/macOS autostart hooks still unset (setting is persisted only); Linux XDG is the first real hook; systemd unit is docs-only.
- GitHub Release still ships `linux/windowsnap` tarballs, not the Tauri binary — **locked** for this PR.
- GNOME tray needs AppIndicator extension; this environment cannot prove tray UX. Linux tray **left-click remains unsupported** in this Tauri/tray-icon pin (menu only); preference removed.
- Region picker is a single fullscreen overlay (primary-oriented); multi-monitor edge cases may need follow-up. Optional `slop` improves X11 selection when installed.
- Annotate is intentionally minimal (pen + rect); no undo stack / arrow / text yet.
