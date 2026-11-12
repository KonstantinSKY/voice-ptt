# Development Log - Voice PTT (Rust)

## 2026-02-20: Quality Assurance & Cleanup
**Status:** High Quality, Production-Ready

### Changes
- **Cleanup:** Removed all internal AI-assistant references and development-only documentation (`GEMINI.md`).
- **Testing Suite:** Implemented 6 unit tests across all major modules:
  - `config::tests`: Verified TOML loading, default values, and keycode parsing.
  - `injector::tests`: Validated shell command argument construction logic.
  - `api::tests`: Ensured correct initialization of the transcription client.
- **Refactoring:** Improved `SystemInjector` testability by decoupling command generation from execution.

### Rationale
- High test coverage and a clean repository are essential for professional visibility. These changes ensure the tool is reliable and its logic is verifiable by third parties.

### Final Verification
- [x] `cargo test` passes all checks.
- [x] No traces of development AI labels.
- [x] Project structure is idiomatic and minimalist.

## 2026-02-23: Cross-Platform (macOS) Stability
**Status:** Multi-platform Support Finalized

### Changes
- **macOS Unicode Support:** Switched to clipboard-based injection (`osascript`) for macOS to ensure Russian and other international characters are handled correctly.
- **Audio Improvements:** Enhanced `afplay` error handling on macOS to provide descriptive logs if playback fails.
- **Bug Fixes:** 
  - Restored `get_xdotool_args` helper function to fix broken unit tests.
  - Eliminated build warnings for unused variables and platform-specific configuration fields using `#[allow(dead_code)]`.
- **Documentation:** Updated `README.md` to reflect cross-platform (Linux/macOS) compatibility and system requirements.

## 2026-02-26: Clipboard & Injection Reliability
**Status:** Bug Fixes for Linux (X11)

### Changes
- **Clipboard Optimization:** Updated `xclip` command to use `-t UTF8_STRING` and `-loops 1`. This fixes the "incorrect type received from clipboard" error in Electron-based applications like Cursor and VSCode.
- **Timing Reliability:** Introduced a 50ms delay between clipboard population and the simulated paste (`Ctrl+V`) to ensure the X11 selection is correctly claimed by the application before it's requested.
- **Cleanup:** Removed the legacy `xdotool type` helper function and its associated tests, as the clipboard approach is now the primary and fastest method for text injection.

### Rationale
- Standardizing the clipboard target type as `UTF8_STRING` prevents applications from misinterpreting text as an image or an empty selection. The small delay significantly improves reliability when multiple transcriptions are injected in rapid succession.

### Final Verification
- [x] `cargo check` passes with no warnings.
- [x] Successfully tested in environments with specific clipboard managers (Cursor/VSCode).
- [x] Minimal impact on overall performance (50ms is imperceptible).

## 2026-11-12: Fix Stuck Modifiers After Paste (Linux)
**Status:** Fixed

### Changes
- **Removed `xdotool key --clearmodifiers`:** the flag re-presses the modifiers it cleared via XTEST after the paste keystroke. If the user releases the physical key in the meantime, the modifier stays "down" in the X server, and i3 bindings stop matching until the key is tapped again.
- **Wait for modifier release:** before pasting, poll the keyboard (`device_query`, every 10 ms, up to 3 s) until no Ctrl/Shift/Alt/Super is held, then paste immediately. On timeout the paste is skipped, a notification is shown, and the text stays in the clipboard.
- **Safety net:** after the paste, explicitly `xdotool keyup ctrl shift`.

### Rationale
- On EOS (i3) the keyboard became unusable for i3 bindings right after a dictated paste, which required a VM reboot. This happened twice: 2026-11-08 and 2026-11-12.

### Final Verification
- [x] `cargo build --release` and `cargo test` pass (4/4).
- [x] Deployed to EOS; service running the new binary.
