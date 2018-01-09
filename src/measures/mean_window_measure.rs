use audio::audio_format::*;
use audio::audio_frame::*;
use pipeline::queue_buf::*;
use measures::*;
use stats::mean;

pub struct MeanWindowMeasure {
    default: f64,
    window: usize
}

impl MeanWindowMeasure {
    pub fn new(window: usize, default: f64) -> MeanWindowMeasure {
        MeanWindowMeasure {
            default: default,
            window: window
        }
    }
}

// impl<T> WrappingMeasure<T> for AudioEdgeMeasure<T> {
//     fn inner(&self) -> &T {
//         &self.baseline
//     }

//     fn inner_mut(&mut self) -> &mut T {
//         &mut self.baseline
//     }
// }

impl<'a> Measure<&'a QueueBuf<f64>, f64> for MeanWindowMeasure {
    fn value(&mut self, buf: &QueueBuf<f64>) -> f64 {
        buf.mean(self.window, self.default)
    }
}