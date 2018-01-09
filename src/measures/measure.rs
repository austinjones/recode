use audio::audio_frame::*;
use video::video_frame::*;

// pub trait AudioFrameProcessor {
//     fn process_audio(&mut self, af: &AudioFrame);
// }

// pub trait VideoFrameProcessor {
//     fn process_video(&mut self, vf: &VideoFrame);
// }

// chunking audio volume
// window buffer
//  mean(edge)
//  mean(avg)
//   edge measure
//     exp decay
//       sigmoid
// edge measure

type MeasureF64 = Measure<TimedData<f64>, TimedData<f64>>;
pub struct TimedData<T> {
    pub data: T,
    pub duration: f64
}

impl TimedData<f64> {
    pub fn zero() -> TimedData<f64> {
        TimedData {
            data: 0f64,
            duration: 0f64
        }
    }
}

pub trait Measure<I, O> {
    fn value(&mut self, I) -> O;
}

pub trait StatefulMeasure<I, O> {
    fn value(&mut self) -> O;
    fn update(&mut self, I);
}

// pub trait Measure<Frame> {
//     fn value(&self) -> f64;
//     fn time(&self) -> f64;
//     fn update(&mut self, frame: &Frame);
// }

// pub trait WrappingMeasure<F, I: Measure<Frame=F>> {
//     fn inner(&self) -> &I;
//     fn inner_mut(&mut self) -> &mut I;
// }

// impl<T: AudioFrameProcessor> AudioFrameProcessor for WrappingMeasure<T> {
//     fn process_audio(&mut self, af: &AudioFrame) {
//         self.inner_mut().process_audio(af);
//     }
// }

// impl<T: VideoFrameProcessor> VideoFrameProcessor for WrappingMeasure<T> {
//     fn process_video(&mut self, vf: &VideoFrame) {
//         self.inner_mut().process_video(vf);
//     }
// }

pub trait PixelTransform {
    fn transform(&self, y: f64, u: f64, v:f64) -> (f64, f64, f64);
}