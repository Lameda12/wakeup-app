# wakeup

A tray app that uses your webcam and keyboard/trackpad activity to catch you dozing or zoning out, then gets loud until you're back. macOS first, Linux after. Design: [DESIGN.md](DESIGN.md).

Status: **M0** (detection spike) ready for you to run; **M1** (the app) built and dogfoodable.

## Layout

```
crates/wakeup-core       the state machine: idle time + face frames in, nudges/alarms out (pure, 35 tests)
crates/wakeup-vision     camera → MediaPipe face models (ONNX) → eye openness (EAR), head pitch, roll
crates/wakeup-platform   OS integration: idle time, volume override, window levels
crates/wakeup-spike      M0 command-line tool
src-tauri/               the tray app (Tauri 2): monitor loop, windows, sound, tray menu
ui/                      alarm + nudge screens (React, Vite)
models/                  ONNX face models, see models/README.md
```

## Run the app (M1)

**Easiest:** open the latest green [CI run](https://github.com/Lameda12/wakeup-app/actions), download the `wakeup-macos` artifact, unzip, then:

```bash
xattr -dr com.apple.quarantine wakeup.app   # unsigned build: clear the download flag
open wakeup.app
```

An eye appears in the menu bar. Grant camera access when asked. Use **Test alarm** in the menu to try the dismiss flow.

**From source:**

```bash
npm ci --prefix ui
ui/node_modules/.bin/tauri dev
```

**Settings** live in `~/Library/Application Support/app.wakeup.desktop/config.json`, created on first launch. Paste the values the M0 session prints (`ear_closed`, `pitch_neutral`, `pitch_down`) and restart the app. Every alarm is logged to `events.jsonl` in the same folder.

Not in M1 yet (M2): settings screen, guided calibration, meeting auto-pause, launch at login, mic detection. Debug switches: `WAKEUP_NO_CAMERA=1`, `WAKEUP_TEST_ALARM_AFTER=5`.

## Run M0 on your Mac

One-time setup:

```bash
xcode-select --install                                   # skip if already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/Lameda12/wakeup-app && cd wakeup-app
git checkout claude/relaxed-keller-agwtr2
```

The first build downloads ONNX Runtime and takes a few minutes. macOS will ask whether your terminal app can use the camera: allow it. If you missed the prompt, enable it under System Settings → Privacy & Security → Camera.

**1. Guided sessions** (about 50s each). Turn your volume up: instructions are spoken, because one step has your eyes closed.

```bash
cargo run --release -p wakeup-spike -- session --tag default   --out default.csv
cargo run --release -p wakeup-spike -- session --tag glasses   --out glasses.csv     # if you wear them
cargo run --release -p wakeup-spike -- session --tag low-light --out low-light.csv   # room lights off, screen only
```

Each run ends with PASS/FAIL checks, a perf line, and values for the app's `config.json`. Send me those printouts plus the CSVs.

**2. Idle time.** Run this, then type for a bit, stop, and watch the counter climb:

```bash
cargo run --release -p wakeup-spike -- idle
```

**Also handy:**

```bash
cargo run --release -p wakeup-spike -- live                     # live readout until Ctrl+C
cargo run --release -p wakeup-spike -- image face.jpg --out annotated.png
```

While a session runs, check Activity Monitor for `wakeup-spike` CPU %, to sanity-check the perf line.

## Develop

```bash
npm ci --prefix ui && npm run build --prefix ui   # the app embeds ui/dist at compile time
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

On a machine that can't download ONNX Runtime at build time, point to a local copy instead:
`ORT_DYLIB_PATH=/path/to/libonnxruntime.so cargo test --workspace --no-default-features --features wakeup-vision/load-dynamic`
