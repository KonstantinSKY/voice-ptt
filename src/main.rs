use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use device_query::{DeviceQuery, DeviceState, Keycode};
use dotenvy::dotenv;
use reqwest::{multipart, Client};
use serde::Deserialize;
use std::env;
use std::fs;
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[derive(Deserialize, Clone)]
struct AppConfig {
    ptt_key: String,
    typing_delay_ms: u64,
    initial_delay_ms: u64,
    model: String,
    language: Option<String>,
    // Sound settings
    sound_enabled: bool,
    sound_start_path: String,
    sound_end_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ptt_key: "RControl".to_string(),
            typing_delay_ms: 50,
            initial_delay_ms: 150,
            model: "whisper-1".to_string(),
            language: None,
            sound_enabled: true,
            sound_start_path: "/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga".to_string(),
            sound_end_path: "/usr/share/sounds/freedesktop/stereo/screen-capture.oga".to_string(),
        }
    }
}

fn play_sound(enabled: bool, path: &str) {
    if enabled {
        let _ = Command::new("paplay").arg(path).spawn();
    }
}

fn notify_send(title: &str, message: &str) {
    let _ = Command::new("notify-send")
        .arg(title)
        .arg(message)
        .arg("-t")
        .arg("5000") // 5 seconds
        .spawn();
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup Environment & Config
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path.parent().context("Failed to get executable directory")?;
    
    // Paths
    let env_path = exe_dir.join(".env");
    let config_path = exe_dir.join("config.toml");

    // Load .env
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
        println!("Loaded .env from {:?}", env_path);
    } else {
        println!("Warning: .env not found at {:?}, checking current dir...", env_path);
        let _ = dotenv(); 
    }
    
    let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY must be set")?;

    // Load config.toml
    let app_config = if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: AppConfig = toml::from_str(&content)?;
        println!("Loaded config from {:?}", config_path);
        config
    } else {
        println!("Config not found at {:?}, using defaults", config_path);
        AppConfig::default()
    };

    // Parse Keycode
    let ptt_key = Keycode::from_str(&app_config.ptt_key).unwrap_or_else(|_| {
        eprintln!("Invalid key '{}', defaulting to RControl", app_config.ptt_key);
        Keycode::RControl
    });

    if Command::new("xdotool").arg("--version").output().is_err() {
        eprintln!("Error: 'xdotool' is required but not found.");
        notify_send("Voice PTT Error", "xdotool not found");
        std::process::exit(1);
    }

    // 2. Audio Setup
    println!("Init audio...");
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device found")?;
    println!(
        "Using input device: {}",
        device.name().unwrap_or("default".into())
    );

    let config = device.default_input_config()?;
    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let audio_buffer = Arc::new(Mutex::new(Vec::new()));
    let writer_buffer = audio_buffer.clone();
    let is_recording = Arc::new(AtomicBool::new(false));
    let reader_is_recording = is_recording.clone();

    // 3. Audio Stream
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if reader_is_recording.load(Ordering::Relaxed) {
                    let mut buffer = writer_buffer.lock().unwrap();
                    for &sample in data {
                        let sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        buffer.push(sample);
                    }
                }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                if reader_is_recording.load(Ordering::Relaxed) {
                    let mut buffer = writer_buffer.lock().unwrap();
                    buffer.extend_from_slice(data);
                }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                if reader_is_recording.load(Ordering::Relaxed) {
                    let mut buffer = writer_buffer.lock().unwrap();
                    for &sample in data {
                        let sample = (sample as i32 - 32768) as i16;
                        buffer.push(sample);
                    }
                }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        )?,
        _ => anyhow::bail!("Unsupported sample format"),
    };

    stream.play()?;

    // 4. Main Loop
    let device_state = DeviceState::new();
    println!("Ready! Hold [{}] to speak.", ptt_key);

    // Clone config for async usage
    let config_clone = app_config.clone();

    loop {
        let keys = device_state.get_keys();

        if keys.contains(&ptt_key) {
            if !is_recording.load(Ordering::Relaxed) {
                // Key Press: Start Recording
                play_sound(config_clone.sound_enabled, &config_clone.sound_start_path);
                println!("\nRecording...");
                audio_buffer.lock().unwrap().clear();
                is_recording.store(true, Ordering::Relaxed);
            }
        } else {
            if is_recording.load(Ordering::Relaxed) {
                // Key Release: Stop Recording & Process
                is_recording.store(false, Ordering::Relaxed);
                play_sound(config_clone.sound_enabled, &config_clone.sound_end_path);
                println!("Processing...");

                let buffer_snapshot: Vec<i16> = {
                    let buf = audio_buffer.lock().unwrap();
                    buf.clone()
                };

                if buffer_snapshot.is_empty() {
                    println!("Buffer empty, ignoring.");
                    continue;
                }

                // Call API
                if let Err(e) = process_audio(buffer_snapshot, spec, &api_key, &config_clone).await {
                    eprintln!("Error processing audio: {}", e);
                    notify_send("Voice PTT Error", &e.to_string());
                }

                println!("\nReady! Hold [{}] to speak.", ptt_key);
            }
        }

        thread::sleep(Duration::from_millis(50));
    }
}

async fn process_audio(buffer: Vec<i16>, spec: hound::WavSpec, api_key: &str, config: &AppConfig) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let file_path = temp_dir.path().join("recording.wav");

    {
        let mut writer = hound::WavWriter::create(&file_path, spec)?;
        for sample in buffer {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
    }

    let client = Client::new();
    let file_content = tokio::fs::read(&file_path).await?;
    let part = multipart::Part::bytes(file_content)
        .file_name("recording.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .text("model", config.model.clone())
        .part("file", part);

    if let Some(lang) = &config.language {
        form = form.text("language", lang.clone());
    }

    let res = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("API Error: {}", error_text);
    }

    let response_data: TranscriptionResponse = res.json().await?;
    let text = response_data.text.trim();

    if !text.is_empty() {
        println!("Transcribed: '{}'", text);
        tokio::time::sleep(Duration::from_millis(config.initial_delay_ms)).await;

        Command::new("xdotool")
            .arg("type")
            .arg("--clearmodifiers")
            .arg("--delay")
            .arg(config.typing_delay_ms.to_string())
            .arg(text)
            .status()
            .context("Failed to execute xdotool")?;
    } else {
        println!("No text detected.");
    }

    Ok(())
}