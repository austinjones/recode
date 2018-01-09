
use audio::audio_frame::*;
use measures::*;

#[derive(Clone)]
pub struct ChunkMeasure {
    value: f64,
    chunk_sum: f64,
    chunk_time: f64,
    chunk_size: usize,
    window_time: f64
}

impl ChunkMeasure {
    pub fn new(window: f64) -> AudioAbsChunkMeasure {
        AudioAbsChunkMeasure {
            value: ::std::f64::NAN;
            chunk_sum: 0f64,
            chunk_sum: 0f64,
            chunk_size: 0,
            window_time: 0f64
        }
    }
}

impl<'a> MeasureF64 for ChunkMeasure {
    fn value(&mut self) -> f64 {
        if self.value.is_nan() {
            self.avg()
        } else {
            self.value
        }
    }

    fn avg(&self) -> f64 {
        self.chunk_sum / (self.chunk_size as f64)
    }

    fn update(&mut self, input: f64) {
        self.chunk_sum += input;
        self.time += af.format.frame_duration;
        self.chunk_size += 1;

        if self.time > self.window_time {
            self.value = self.avg();
        }
    }
}