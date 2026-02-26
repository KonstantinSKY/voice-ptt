use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AudioSystem {
    pub device: cpal::Device,
    pub config: cpal::SupportedStreamConfig,
}

impl AudioSystem {
    /// Initializes the default audio input device and its configuration.
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device found. Please check your microphone connection.")?;

        let config = device
            .default_input_config()
            .context("Failed to get default input configuration")?;

        Ok(Self { device, config })
    }

    /// Returns a WAV specification based on the current device configuration.
    pub fn get_wav_spec(&self) -> hound::WavSpec {
        hound::WavSpec {
            channels: self.config.channels(),
            sample_rate: self.config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }
    }

    /// Builds an input stream that captures audio into the provided buffer when `is_recording` is true.
    pub fn build_stream(
        &self,
        audio_buffer: Arc<Mutex<Vec<i16>>>,
        is_recording: Arc<AtomicBool>,
    ) -> Result<cpal::Stream> {
        let writer_buffer = audio_buffer;
        let reader_is_recording = is_recording;

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::F32 => self.device.build_input_stream(
                &self.config.clone().into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if reader_is_recording.load(Ordering::Relaxed) {
                        if let Ok(mut buffer) = writer_buffer.lock() {
                            for &sample in data {
                                let sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                buffer.push(sample);
                            }
                        }
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            ),
            cpal::SampleFormat::I16 => self.device.build_input_stream(
                &self.config.clone().into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if reader_is_recording.load(Ordering::Relaxed) {
                        if let Ok(mut buffer) = writer_buffer.lock() {
                            buffer.extend_from_slice(data);
                        }
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            ),
            _ => anyhow::bail!(
                "Unsupported audio sample format. Only F32 and I16 are currently supported."
            ),
        }?;

        Ok(stream)
    }
}
