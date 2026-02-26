use crate::config::AppConfig;
use anyhow::{Context, Result};
use reqwest::{multipart, Client};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub struct WhisperClient {
    client: Client,
    api_key: String,
}

impl WhisperClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    /// Sends the audio buffer to OpenAI Whisper API for transcription.
    pub async fn transcribe(
        &self,
        buffer: Vec<i16>,
        spec: hound::WavSpec,
        config: &AppConfig,
    ) -> Result<String> {
        let temp_dir = tempfile::tempdir()?;
        let file_path = temp_dir.path().join("recording.wav");

        // Write WAV file
        {
            let mut writer = hound::WavWriter::create(&file_path, spec)
                .context("Failed to create WAV writer")?;
            for sample in buffer {
                writer.write_sample(sample)?;
            }
            writer.finalize()?;
        }

        self.transcribe_wav_file(&file_path, config).await
    }

    pub async fn transcribe_wav_file(&self, file_path: &Path, config: &AppConfig) -> Result<String> {
        let file_content = tokio::fs::read(file_path)
            .await
            .with_context(|| format!("Failed to read WAV file at {}", file_path.display()))?;
        self.transcribe_wav_bytes(file_content, config).await
    }

    async fn transcribe_wav_bytes(&self, file_content: Vec<u8>, config: &AppConfig) -> Result<String> {
        let part = multipart::Part::bytes(file_content)
            .file_name("recording.wav")
            .mime_str("audio/wav")?;

        let mut form = multipart::Form::new()
            .text("model", config.model.clone())
            .part("file", part);

        if let Some(lang) = &config.language {
            form = form.text("language", lang.clone());
        }

        let res = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("Failed to send request to OpenAI")?;

        if !res.status().is_success() {
            let error_text = res.text().await?;
            anyhow::bail!("OpenAI API Error: {}", error_text);
        }

        let response_data: TranscriptionResponse = res
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        Ok(response_data.text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_client_init() {
        let key = "sk-test-key".to_string();
        let client = WhisperClient::new(key.clone());
        assert_eq!(client.api_key, key);
    }
}
