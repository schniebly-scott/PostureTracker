# Application icon

The icon the OS shows *for the app* — Explorer / the Windows taskbar, the macOS
Dock and Finder, the window's own title bar. This is separate from the tray icon
in `../tray`, which is a small monochrome mark drawn into the system tray.

Three different consumers need three different formats, and each one has to be
wired up separately — shipping only one of them is why the icon shows up in some
places and not others:

| File | Consumer | Wired up in |
| --- | --- | --- |
| `app-icon-256.png` | The live window (Windows title bar + taskbar, Linux dock) | `include_bytes!` in `app::window_icon` |
| `app-icon.ico` | The `.exe` itself (Explorer, Start menu, pinned taskbar entries) | `build.rs`, as a Win32 resource |
| `app-icon.icns` | The macOS `.app` bundle (Dock, Finder) | `CFBundleIconFile` in the release workflow's `Info.plist` |

macOS ignores the window icon entirely, so the `.icns` is the only thing that
makes the Dock and Finder show the mark — it is not a nicer alternative to
`window_icon`, it is the mechanism. Both `.ico` and `.icns` are multi-resolution
containers; the shells pick a size themselves, so all the `app-icon-<n>.png`
renders are kept here as their sources.

macOS aggressively caches bundle icons. After replacing the `.icns`, a stale Dock
or Finder icon usually means the cache, not the asset.

## Reproducing these files

`app-icon.svg` is the master (256×256, rounded-rect). The PNGs are straight
renders of it:

```sh
for s in 16 24 32 48 64 128 256 512 1024; do
  rsvg-convert -w $s -h $s app-icon.svg -o app-icon-$s.png
done
```

`app-icon.icns` holds those PNGs verbatim under the `icp4`/`icp5`/`icp6` and
`ic07`…`ic10` types (16–1024 px); `iconutil -c icns` on macOS produces the same
thing from an `.iconset` directory.

`app-icon.ico` deliberately stores its sub-256 entries as classic 32-bpp BMP/DIB
(bottom-up BGRA + a 1-bpp AND mask) and only the 256 px entry as PNG. Windows
does read PNG-compressed entries, but the BMP layout is what every shell surface
has always understood, and the size cost at these dimensions is trivial. Note
that a BMP entry's header records *double* the real height, to account for the
mask rows that follow the colour rows.
