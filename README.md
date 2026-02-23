# Voice PTT (Rust)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/yourusername/voice-ptt)

**Voice PTT** is a high-performance, low-latency Push-to-Talk (PTT) dictation tool built in Rust. It enables seamless voice-to-text input across **Linux (X11)** and **macOS** applications by capturing audio via global hotkeys and transcribing it using OpenAI's Whisper API.

Unlike heavy Electron-based or Python-scripted alternatives, **Voice PTT** is a compiled binary designed for power users who value speed, minimalism, and system integration.

---

## 🚀 Key Features

- **Global Hotkey Integration:** Native global key state monitoring (uses `device_query`).
- **Low-Latency Audio Pipeline:** Direct microphone access via `cpal` with support for multiple sample formats (F32, I16, U16).
- **Intelligent Transcription:** Leverages OpenAI Whisper `whisper-1` for near-human accuracy in multiple languages.
- **Cross-Platform Text Injection:**
  - **Linux (X11):** Simulates hardware keyboard events via `xdotool`.
  - **macOS:** Uses `osascript` with clipboard integration for perfect Unicode/International support.
- **Non-Blocking Architecture:** Asynchronous I/O powered by `tokio` ensures the UI/system remains responsive during processing.
- **Minimal Footprint:** Single binary with zero background daemon overhead.

## 🛠 How It Works

1. **Detection:** The tool monitors the keyboard state globally. When the configured PTT key is held, the audio capture thread activates.
2. **Capture:** High-fidelity audio is buffered in memory (RAM) to avoid disk I/O latency during recording.
3. **Processing:** Upon key release, the buffer is converted to a compliant WAV format and streamed to the OpenAI API.
4. **Injection:** The returned transcription is "typed" or "pasted" into the currently active window. On macOS, this uses the clipboard to ensure special characters (like Russian or Emoji) are handled correctly.

---

## 📋 Prerequisites

### System Dependencies
You need the following libraries and tools installed on your system:

**Arch / Manjaro:**
```bash
sudo pacman -S xdotool alsa-lib pulseaudio-utils
```

**Ubuntu / Debian:**
```bash
sudo apt update && sudo apt install xdotool libasound2-dev pulseaudio-utils
```

**macOS:**
- No extra dependencies are required (uses built-in `osascript` and `afplay`).
- **Note:** You may need to grant **Accessibility** permissions to your Terminal or IDE in `System Settings > Privacy & Security > Accessibility`.

### API Access
An **OpenAI API Key** is required. Set it in your environment or a `.env` file in the same directory:
```bash
export OPENAI_API_KEY='your-key-here'
```

---

## ⚙️ Installation & Setup

### 1. Build from Source
Ensure you have the Rust toolchain installed (`rustup`).

```bash
git clone https://github.com/yourusername/voice-ptt.git
cd voice-ptt
cargo build --release
```

### 2. Configuration
The application looks for a `config.toml` in the same directory as the binary.

```toml
# config.toml
ptt_key = "RControl"       # Key to hold (RControl, LAlt, LControl, etc.)
typing_delay_ms = 40       # Milliseconds between virtual keystrokes
initial_delay_ms = 100     # Pause before starting to type
model = "whisper-1"        # OpenAI model to use
language = "ru"            # Optional: transcription language (iso-639-1)

# Audio Feedback
sound_enabled = true
sound_start_path = "/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga"
sound_end_path = "/usr/share/sounds/freedesktop/stereo/screen-capture.oga"
```

---

## 🖥 Usage

Simply run the binary:
```bash
./target/release/voice-ptt
```

1. **Press and hold** the PTT key (default: `Right Control`).
2. **Speak** clearly into your microphone.
3. **Release** the key.
4. The transcription will appear automatically at your cursor position.

---

## 🗺 Roadmap

- [ ] **Wayland Support:** Replace `xdotool` with `ydotool` or native portal-based input.
- [ ] **Local LLM Support:** Add backend for local Whisper (via `whisper.cpp`).
- [ ] **Visual Indicator:** Optional overlay/bar icon showing recording state.
- [ ] **Custom Commands:** Map specific phrases to shell commands.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## ⚖️ License

Distributed under the MIT License. See `LICENSE` for more information.
