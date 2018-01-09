use audio::audio_format::*;

pub struct AudioFrame<'a> {
    pub data: &'a mut [i32],
    pub format: &'a AudioFormat,
    pub time: f64
}

impl<'a> AudioFrame<'a> {
    pub fn new(data: &'a mut [i32], format: &'a AudioFormat, time: f64) -> AudioFrame<'a> {
        AudioFrame {data: data, format:format, time: time}
    }

    pub fn sum(&self) -> f64 {
        self.data.iter().fold(0f64, |a,b| a+(*b as f64))
    }

    pub fn abs_sum(&self) -> f64 {
        self.data.iter().map(|e| e.abs() as f64).fold(0f64, |a,b| a+b)
    }
}