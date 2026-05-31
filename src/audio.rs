use std::io::{Cursor, Read};

/// Fetch audio bytes from a URL.
fn fetch_audio_bytes(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Failed to fetch audio: {}", e))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read audio data: {}", e))?;

    if bytes.is_empty() {
        return Err("Received empty audio data.".to_string());
    }

    Ok(bytes)
}

/// Play audio from MP3 bytes through the default audio device.
fn play_bytes(mp3_data: Vec<u8>) -> Result<(), String> {
    use rodio::{Decoder, DeviceSinkBuilder, Player};

    let handle = DeviceSinkBuilder::open_default_sink()
        .map_err(|e| format!("Audio device error: {}", e))?;

    let source = Decoder::try_from(Cursor::new(mp3_data))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    let player = Player::connect_new(&handle.mixer());
    player.append(source);
    player.sleep_until_end();

    Ok(())
}

/// Fetch and play pronunciation audio from a URL.
pub fn play_pronunciation(url: &str) -> Result<(), String> {
    let bytes = fetch_audio_bytes(url)?;
    play_bytes(bytes)
}
