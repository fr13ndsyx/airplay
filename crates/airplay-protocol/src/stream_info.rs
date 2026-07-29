#![allow(clippy::pedantic)]

/// Type of media stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Audio,
    Video,
}

/// Information about a video stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStreamInfo {
    pub stream_connection_id: String,
}

impl VideoStreamInfo {
    pub fn new(stream_connection_id: String) -> Self {
        Self {
            stream_connection_id,
        }
    }
}

impl std::fmt::Display for VideoStreamInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VideoStreamInfo{{streamConnectionId='{}'}}",
            self.stream_connection_id
        )
    }
}

/// Audio compression type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum CompressionType {
    Lpcm = 1,
    Alac = 2,
    Aac = 4,
    AacEld = 8,
    Opus = 32,
}

impl CompressionType {
    pub fn code(&self) -> u64 {
        *self as u64
    }

    pub fn from_code(code: u64) -> Option<Self> {
        match code {
            1 => Some(Self::Lpcm),
            2 => Some(Self::Alac),
            4 => Some(Self::Aac),
            8 => Some(Self::AacEld),
            32 => Some(Self::Opus),
            _ => None,
        }
    }
}

/// Audio format describing sample rate, bit depth, and channel count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum AudioFormat {
    Pcm8000_16_1 = 0x4,
    Pcm8000_16_2 = 0x8,
    Pcm16000_16_1 = 0x10,
    Pcm16000_16_2 = 0x20,
    Pcm24000_16_1 = 0x40,
    Pcm24000_16_2 = 0x80,
    Pcm32000_16_1 = 0x100,
    Pcm32000_16_2 = 0x200,
    Pcm44100_16_1 = 0x400,
    Pcm44100_16_2 = 0x800,
    Pcm44100_24_1 = 0x1000,
    Pcm44100_24_2 = 0x2000,
    Pcm48000_16_1 = 0x4000,
    Pcm48000_16_2 = 0x8000,
    Pcm48000_24_1 = 0x10000,
    Pcm48000_24_2 = 0x20000,
    Alac44100_16_2 = 0x40000,
    Alac44100_24_2 = 0x80000,
    Alac48000_16_2 = 0x100000,
    Alac48000_24_2 = 0x200000,
    AacLc44100_2 = 0x400000,
    AacLc48000_2 = 0x800000,
    AacEld44100_2 = 0x1000000,
    AacEld48000_2 = 0x2000000,
    AacEld16000_1 = 0x4000000,
    AacEld24000_1 = 0x8000000,
    Opus16000_1 = 0x10000000,
    Opus24000_1 = 0x20000000,
    Opus48000_1 = 0x40000000,
    AacEld44100_1 = 0x80000000,
    AacEld48000_1 = 0x100000000,
}

impl AudioFormat {
    pub fn code(&self) -> u64 {
        *self as u64
    }

    pub fn from_code(code: u64) -> Option<Self> {
        match code {
            0x4 => Some(Self::Pcm8000_16_1),
            0x8 => Some(Self::Pcm8000_16_2),
            0x10 => Some(Self::Pcm16000_16_1),
            0x20 => Some(Self::Pcm16000_16_2),
            0x40 => Some(Self::Pcm24000_16_1),
            0x80 => Some(Self::Pcm24000_16_2),
            0x100 => Some(Self::Pcm32000_16_1),
            0x200 => Some(Self::Pcm32000_16_2),
            0x400 => Some(Self::Pcm44100_16_1),
            0x800 => Some(Self::Pcm44100_16_2),
            0x1000 => Some(Self::Pcm44100_24_1),
            0x2000 => Some(Self::Pcm44100_24_2),
            0x4000 => Some(Self::Pcm48000_16_1),
            0x8000 => Some(Self::Pcm48000_16_2),
            0x10000 => Some(Self::Pcm48000_24_1),
            0x20000 => Some(Self::Pcm48000_24_2),
            0x40000 => Some(Self::Alac44100_16_2),
            0x80000 => Some(Self::Alac44100_24_2),
            0x100000 => Some(Self::Alac48000_16_2),
            0x200000 => Some(Self::Alac48000_24_2),
            0x400000 => Some(Self::AacLc44100_2),
            0x800000 => Some(Self::AacLc48000_2),
            0x1000000 => Some(Self::AacEld44100_2),
            0x2000000 => Some(Self::AacEld48000_2),
            0x4000000 => Some(Self::AacEld16000_1),
            0x8000000 => Some(Self::AacEld24000_1),
            0x10000000 => Some(Self::Opus16000_1),
            0x20000000 => Some(Self::Opus24000_1),
            0x40000000 => Some(Self::Opus48000_1),
            0x80000000 => Some(Self::AacEld44100_1),
            0x100000000 => Some(Self::AacEld48000_1),
            _ => None,
        }
    }
}

/// Information about an audio stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioStreamInfo {
    pub compression_type: Option<CompressionType>,
    pub audio_format: Option<AudioFormat>,
    pub samples_per_frame: Option<i32>,
}

impl AudioStreamInfo {
    pub fn builder() -> AudioStreamInfoBuilder {
        AudioStreamInfoBuilder::default()
    }
}

/// Builder for [`AudioStreamInfo`].
#[derive(Debug, Clone, Default)]
pub struct AudioStreamInfoBuilder {
    compression_type: Option<CompressionType>,
    audio_format: Option<AudioFormat>,
    samples_per_frame: Option<i32>,
}

impl AudioStreamInfoBuilder {
    pub fn compression_type(mut self, compression_type: CompressionType) -> Self {
        self.compression_type = Some(compression_type);
        self
    }

    pub fn audio_format(mut self, audio_format: AudioFormat) -> Self {
        self.audio_format = Some(audio_format);
        self
    }

    pub fn samples_per_frame(mut self, samples_per_frame: i32) -> Self {
        self.samples_per_frame = Some(samples_per_frame);
        self
    }

    pub fn build(self) -> AudioStreamInfo {
        AudioStreamInfo {
            compression_type: self.compression_type,
            audio_format: self.audio_format,
            samples_per_frame: self.samples_per_frame,
        }
    }
}

/// Media stream info - either audio or video.
#[derive(Debug, Clone)]
pub enum MediaStreamInfo {
    Audio(AudioStreamInfo),
    Video(VideoStreamInfo),
}

impl MediaStreamInfo {
    pub fn stream_type(&self) -> StreamType {
        match self {
            Self::Audio(_) => StreamType::Audio,
            Self::Video(_) => StreamType::Video,
        }
    }
}
