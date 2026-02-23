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

### Rationale
- The tool is now fully usable on both Linux (X11) and macOS with equivalent functionality and consistent audio feedback.
