# ShotCraft

Lightweight, fast cross-platform screenshot and live screen-annotation app written in Rust (`winit` + `egui` + `glow`).

## Features

- **4 Capture & Annotation Modes** (customizable global hotkeys):
  - **Fullscreen** (`Opt + Shift + 1` / `Alt + Shift + 1`) — instant capture, copy to clipboard & save.
  - **Area** (`Opt + Shift + 2` / `Alt + Shift + 2`) — select any region across multiple monitors and annotate.
  - **Window** (`Opt + Shift + 3` / `Alt + Shift + 3`) — hover and capture any window.
  - **Screen Draw** (`Opt + Shift + 4` / `Alt + Shift + 4`) — draw directly over live video, calls, or any app on a transparent always-on-top canvas, then capture with `1`/`2`/`3` or Copy/Save.
- **Full Annotation Toolkit:** Move/Resize, Pen, Highlighter Marker, Line, Arrow (with configurable `Shift` angle snap), Text & Callout (with `GOST Type AU` and `Roboto` fonts), Ellipse, Rectangle, Step Numbering, and Gaussian Blur.
- **Native Look & Feel:** Damped spring animations, morphing pill/squircle controls, draggable horizontal/vertical floating toolbar, customizable theme colors, and 10 UI languages.

## Downloads

Pre-built installers for **macOS** (`ShotCraft-v0.5.dmg`), **Windows** (`ShotCraft-v0.5-Setup.exe`), and **Linux** (`ShotCraft-v0.5-linux-amd64.deb` / `ShotCraft-v0.5-linux-x86_64.tar.gz`) are available on the [Releases](../../releases) page.

## Build from Source

```bash
# Run tests
cargo test

# Build release binary
cargo build --release

# Package macOS DMG
./package_dmg.sh

# Package Linux (.deb & .tar.gz)
./package_linux.sh

# Cross-compile & package Windows NSIS installer (requires x86_64-pc-windows-gnu & makensis)
./package_windows.sh
```
