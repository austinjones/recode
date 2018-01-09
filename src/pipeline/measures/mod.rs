use measures::log_ratio_measure::*;
use measures::exp_decay_measure::*;
use measures::mean_window_measure::*;
use measures::sigmoid_measure::*;
use measures::mean_measure::*;

use audio::audio_format::*;
use audio::audio_frame::*;

use measures::*;

use pipeline::queue_buf::*;

use rustfft::num_complex::*;

use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use rustfft::FFT;
use apodize::{hanning_iter};

pub struct NormalizedAudioEdgeMeasure {
    buf: QueueBuf<f64>,
    edge_window: MeanWindowMeasure,
    edge_avg: MeanWindowMeasure,
    ratio: LogRatioMeasure,
    decay: ExpDecayMeasure,
    sigmoid: SigmoidMeasure,
    time: f64
}

impl NormalizedAudioEdgeMeasure {
    pub fn new(af: &AudioFormat) -> NormalizedAudioEdgeMeasure {
        let edge_frames = af.frames_in(0.022f64);
        let avg_frames = af.frames_in(0.45f64);
        let buf_len = ::std::cmp::max(edge_frames, avg_frames);
        let vec = vec!(0f64; buf_len);

        NormalizedAudioEdgeMeasure {
            buf: QueueBuf::new(vec),
            edge_window: MeanWindowMeasure::new(edge_frames, 0f64),
            edge_avg: MeanWindowMeasure::new(avg_frames, 0f64),
            ratio: LogRatioMeasure::new(),
            decay: ExpDecayMeasure::new(0.04f64, 0.16f64),
            sigmoid: SigmoidMeasure::new(SigmoidRange::NegOneToOne),
            time: 0f64
        }
    }

    pub fn update(&mut self, frame: &AudioFrame) {
        self.buf.push(frame.abs_sum());
        self.time = frame.time;
    }
}

impl Measure<(), f64> for NormalizedAudioEdgeMeasure {
    fn value(&mut self, none:()) -> f64 {
        let edge = self.edge_window.value(&self.buf);
        let avg = self.edge_avg.value(&self.buf);
        let ratio = self.ratio.value((edge, avg));

        self.decay.update((ratio, self.time));
        let decayed = self.decay.value();
        self.sigmoid.value(decayed)
    }
}

pub struct NormalizedAudioVolumeMeasure {
    buf: QueueBuf<f64>,
    edge: MeanWindowMeasure,
    avg: MeanMeasure,
    ratio: LogRatioMeasure,
    sigmoid: SigmoidMeasure
}

impl NormalizedAudioVolumeMeasure {
    pub fn new(af: &AudioFormat) -> NormalizedAudioVolumeMeasure {
        let edge_frames = 512;
        let vec = vec!(0f64; edge_frames);

        NormalizedAudioVolumeMeasure {
            buf: QueueBuf::new(vec),
            edge: MeanWindowMeasure::new(edge_frames, 0f64),
            avg: MeanMeasure::new(),
            ratio: LogRatioMeasure::new(),
            sigmoid: SigmoidMeasure::new(SigmoidRange::NegOneToOne)
        }
    }

    pub fn update(&mut self, frame: &AudioFrame) {
        let sum = frame.abs_sum();
        self.buf.push(sum);
        self.avg.update(sum);
    }
}

impl Measure<(), f64> for NormalizedAudioVolumeMeasure {
    fn value(&mut self, _:()) -> f64 {
        let edge = self.edge.value(&self.buf);
        let avg = self.avg.value();
        let ratio = self.ratio.value((edge, avg));
        self.sigmoid.value(ratio)
    }
}

pub struct FFTMeasure {
    buf: QueueBuf<f64>,
    window: Vec<f64>,
    fft_size: usize,
    buckets: usize,
    bin_size_hz: f64
}

impl FFTMeasure {
    pub fn new(af: &AudioFormat, buckets: usize) -> FFTMeasure {
        let fft_size = 8192;
        println!("fft size: {}", fft_size);
        FFTMeasure {
            buf: QueueBuf::new(vec!(0f64; fft_size)),
            window: hanning_iter(fft_size).collect(),
            fft_size: fft_size,
            buckets: buckets,
            bin_size_hz: (af.rate as f64) / (fft_size as f64 / 2f64)
        }
    }
}

impl<'a, 'b> StatefulMeasure<&'a AudioFrame<'b>, Vec<f64>> for FFTMeasure {
    fn update(&mut self, frame: &AudioFrame) {
        self.buf.push(frame.sum());
    }

    fn value(&mut self) -> Vec<f64> {
        let raw_input = self.buf.extract();
        let raw_input: Vec<f64> = raw_input.iter().zip(self.window.iter()).map(|(x, y)| x*y).collect();

        let mut output = vec!(Complex64::zero(); self.fft_size);
        let mut input: Vec<Complex64> = raw_input.into_iter()
            .map(|e| Complex::new(e, 0f64)).collect();

        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft(self.fft_size);
        fft.process(&mut input, &mut output);

        // minimum human hearing is 20hz.  any lower frequencies aren't useful for visualization
        // cut them off.
        let min_bin = (20f64 / self.bin_size_hz).ceil() as usize;
        // fft output has a complex symmetry.  real squares are symmetric
        // last useful bin is fft_size / 2 - 1, but rust ranges have exclusive high endpoint
        // we want to constrain the range though, because higher sample frequencies increase bin sizes.
        // if we get 128khz audio, we don't want the output to change.
        let max_bin_from_hz = (8000f64 / self.bin_size_hz).ceil() as usize;
        let max_bin = ::std::cmp::min(self.fft_size/2, max_bin_from_hz);
        let output: Vec<f64> = output[min_bin..max_bin].into_iter()
            .map(|e| e.norm_sqr().sqrt()).collect();

        let min_freq = (min_bin as f64) * self.bin_size_hz;
        let max_freq = (max_bin as f64) * self.bin_size_hz;
        
        let sum = output.iter().fold(0f64, |a,b| a+b);
        let scale = (output.len() as f64) / sum;

        let output: Vec<f64> = output.into_iter()
            .map(|e| scale * e).collect();

        println!("A: {}, B:{}", output.len(), self.buckets);
        let output: Vec<f64> = output.chunks(output.len() / self.buckets)
            .map(|e| ::stats::mean(e.iter().map(|e| *e)))
            .collect();

        let output: Vec<f64> = output.iter()
            .map(|e| SigmoidMeasure::sigmoid(2f64*e.ln()))
            .collect();
        
        // println!("Before remap: {:?}", output);

        println!("{} FFT bins from {:.2}Hz to {:.2}Hz", output.len(), min_freq, max_freq);
        for v in output.iter() {
            let v = *v;
            if v <= 0.01f64 {
                print!(" ");
            } else if v <= 0.2f64 {
                print!(".")
            } else if v <= 0.4f64 {
                print!("o")
            } else if v <= 0.6f64 {
                print!("e")
            } else if v <= 0.8f64 {
                print!("0")
            } else {
                print!("#")
            }
        }

        println!("");

        output
    }
}



// pub type NormalizedAudioVolumeMeasure = SigmoidMeasure<MeanWindowMeasure<AudioVolumeMeasure>>;
// pub fn measure_audio_volume(af: &AudioFormat) -> SigmoidMeasure<MeanWindowMeasure<AudioVolumeMeasure>> {
//     let vol = AudioVolumeMeasure::new();
//     let mean_window = MeanWindowMeasure::new(vol, 0f64, af.frames_in(0.2f64));
//     SigmoidMeasure::new(mean_window, SigmoidRange::NegOneToOne)
// }