#[derive(Copy, Clone, Debug)]
pub struct AudioFormat {
    pub rate: i32,
    pub channels: i32,
    pub frame_size: usize,
    pub frame_duration: f64
}

impl AudioFormat {
    pub fn empty() -> AudioFormat {
        AudioFormat {
            rate: 0,
            channels: 0,
            frame_size: 0,
            frame_duration: 0f64
        }
    }

    pub fn new(rate: i32, channels: i32) -> AudioFormat {
        AudioFormat {
            rate: rate,
            channels: channels,
            frame_size: channels as usize,
            frame_duration: 1f64 / ((rate as f64) * (channels as f64))
        }
    }
}

impl AudioFormat {

}
