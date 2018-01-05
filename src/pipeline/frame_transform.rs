use audio::audio_frame::*;
use video::video_frame::*;


use rustfft::num_complex::*;
use rustfft::num_traits::Zero;
use rustfft::FFTplanner;
use rustfft::FFT;

use pipeline::queue_buf::*;
use std::sync::Arc;

pub trait FrameTransform {
    fn process_audio_frame(&mut self, aframe: &mut AudioFrame, atime: f64);

    fn process_video_frame(&mut self, vframe: &mut VideoFrame, vtime: f64);
}

const AUDIO_SIZE: usize = 1000;
const BASELINE_SIZE: usize = 20000;
const DISTURB_SIZE: usize = 3;
const ABS_VOL_SIZE: usize = 30*3;
const ROTATION_RATE: f64 = (1f64 / 10f64);

pub struct FrameTransformImpl {
    queue_buf: QueueBuf<f64>,
    baseline_buf: QueueBuf<f64>,
    disturb_buf: QueueBuf<f64>,
    abs_vol_buf: QueueBuf<f64>,
    max_vol: f64,
    volratio_decay: f64,
    angle: f64,
    fft: Arc<FFT<f64>>
}

impl FrameTransformImpl {
    pub fn new() -> FrameTransformImpl{ 
        
        let mut planner = FFTplanner::new(false);

        FrameTransformImpl {
            queue_buf: QueueBuf::new(vec!(0f64; AUDIO_SIZE)),
            baseline_buf: QueueBuf::new(vec!(0f64; BASELINE_SIZE)),
            disturb_buf: QueueBuf::new(vec!(0f64; DISTURB_SIZE)),
            abs_vol_buf: QueueBuf::new(vec!(0f64; ABS_VOL_SIZE)),
            max_vol: 0f64,
            volratio_decay: 0f64,
            angle: 0f64,
            fft: planner.plan_fft(AUDIO_SIZE)
        }
    }

    fn rotate(y:u8, u: u8, v: u8, r: f64, s: f64) -> (u8, u8) {
        let r_adjust = (y as f64 - 128f64) / (128f64);
        // let r_sin = (r_adjust * 64f64).sin();
        let rads = (r + r_adjust / 64f64) * 2f64 * ::std::f64::consts::PI;
        let cos = rads.cos();
        let sin = rads.sin();
        let uf = (u as f64) - 128f64;
        let vf = (v as f64) - 128f64;
        let uf_new = uf * cos - vf * sin;
        let vf_new = vf * cos + uf * sin;
        
        let mut uf_scaled = uf_new * s;
        let mut vf_scaled = vf_new * s;

        if uf_scaled < -128f64 {
            uf_scaled = -128f64;
        } else if uf_scaled > 127f64 {
            uf_scaled = 127f64;
        }

        if vf_scaled < -128f64 {
            vf_scaled = -128f64;
        } else if vf_scaled > 127f64 {
            vf_scaled = 127f64;
        }
        
        let uf_new_u8 = (uf_scaled + 128f64) as u8;
        let vf_new_u8 = (vf_scaled + 128f64) as u8;
        (uf_new_u8, vf_new_u8)
    }

    fn sigmoid(x: f64) -> f64 {
        1f64 / (1f64 + ::std::f64::consts::E.powf(-1f64 * x))
    }

    fn remap(x: u8, min: u8, max:u8) -> u8 {
        let xf = x as f64;
        let df = (max-min) as f64;
        let outf = df * (xf / 255f64) + (min as f64);
        outf as u8
    }

    fn center_of_mass(x: &[f64]) -> f64 {
        let m = x.iter().fold(0f64, |a,b| a+b);
        if m == 0f64 {
            return 0f64;
        }

        let r_scale = x.len() as f64;

        let mut i = 1;
        let mut sum = 0f64;
        for m_i in x {
            let r_i = i as f64 / r_scale;
            
            sum += m_i * r_i;
            i += 1;
        }

        sum / m
    }

    fn sigmoid_remap(x: u8, div: f64) -> u8 {
        let xf = x as f64;
        let sig = Self::sigmoid(xf / div) - 0.5f64;
        (sig * 2f64 * 255f64) as u8
    }

    fn avg(x: &[f64]) -> f64 {
        let total = x.iter().fold(0f64, |a,b| a+b);
        let n = x.len() as f64;

        total/n
    }
}

impl FrameTransform for FrameTransformImpl {
    fn process_audio_frame(&mut self, aframe: &mut AudioFrame, atime: f64) {
        let sum = aframe.sum();
        self.queue_buf.push(sum);
        self.baseline_buf.push(sum);
    }


    fn process_video_frame(&mut self, vframe: &mut VideoFrame, vtime: f64) {
        // let mut fft_output = vec!(Complex64::zero(); FFT_SIZE);
        let audio_vec = self.queue_buf.extract();
        let abs_audio: Vec<f64> = audio_vec.iter().map(|e| e.abs()).collect();

        let baseline_vec = self.baseline_buf.extract();
        let abs_baseline: Vec<f64> = baseline_vec.iter().map(|e| e.abs()).collect();

        let avg_vol = Self::avg(abs_audio.as_slice());
        let baseline_vol = Self::avg(abs_baseline.as_slice());
        if avg_vol > self.max_vol {
            self.max_vol = avg_vol;
        }

        let abs_vol = if self.max_vol > 0f64 { avg_vol / self.max_vol } else { 0f64 };
        self.abs_vol_buf.push(abs_vol);

        let avg_abs_vol = Self::avg(self.abs_vol_buf.extract().as_slice());
        let raw_rotation = (1f64+3f64*abs_vol) * ROTATION_RATE;
        // self.fft.process(audio_vec.as_mut_slice(), fft_output.as_mut_slice());
        // let fft_output: Vec<f64> = fft_output[0..FFT_SIZE/2].iter().map(|e| e.norm_sqr()).collect();
        // let center_of_mass = Self::center_of_mass(fft_output.as_slice());
        // let avg = fft_output.iter().fold(0f64, |a,b| a+b) / (fft_output.len() as f64);
        // let fft_output: Vec<f64> = fft_output.iter().map(|e| e / avg).collect();

        // let low = fft_output[0..FFT_SIZE/4].iter().fold(0f64, |a,b| a+b) / (FFT_SIZE as f64 / 2f64);
        // let high = fft_output[FFT_SIZE/4..FFT_SIZE/2].iter().fold(0f64, |a,b| a+b) / (FFT_SIZE as f64 / 2f64);
        // println!("Low: {}, High: {}", low, high);

        // let rotation = (vtime % 7f64) / 7f64;
        println!("Avg vol: {}, Max Vol: {}, vol ratio: {}, avg ratio: {}", avg_vol, self.max_vol, abs_vol, avg_abs_vol);
        self.angle += raw_rotation * vframe.format.frame_duration;

        let spike_volratio = if baseline_vol == 0f64 { 1f64 } else { avg_vol/baseline_vol };
        if spike_volratio > self.volratio_decay {
            self.volratio_decay = self.volratio_decay + 0.6f64 * (spike_volratio - self.volratio_decay);
        } else {
            self.volratio_decay = self.volratio_decay - 0.2f64 * (self.volratio_decay - spike_volratio);
        }

        let raw_disturb = self.volratio_decay-1f64;
        // self.disturb_buf.push(raw_disturb);
        let disturbance = Self::sigmoid(2f64*(self.volratio_decay-1f64))-0.5f64;
        println!("Vol_Decay: {}, disturb: {}", self.volratio_decay, disturbance);
        // let disturbance = Self::avg(self.disturb_buf.extract().as_slice());
        // let disturbance = Self::sigmoid(5f64*disturbance)-0.5f64;
        // let disturbance = Self::sigmoid(5f64*(self.abs_vol_decay - avg_abs_vol))-0.5f64;
        println!("Rotation: {}, raw disturb: {}, disturbance: {}", raw_rotation, raw_disturb, disturbance);
        // let rotation = Self::sigmoid(15f64 * (high - low))-0.5f64);

        
        // println!("Low: {}, High: {}", low, high);
        // println!("Diff: {}, Rotation: {}", (high-low), rotation);

        if self.queue_buf.is_saturated() {
            // println!("FFT input: {:?}", audio_vec);
            // println!("FFT output: {:?}", fft_output);
        }

        for pixel in vframe.data.chunks_mut(4) {
            let (u, v) = Self::rotate(pixel[1], pixel[2], pixel[3], 
                self.angle + (0.45f64 * disturbance), 
                1.5f64 + 1.1f64 * disturbance);
            // pixel[2] = pixel[2].wrapping_add(rotation);
            
            pixel[1] = Self::sigmoid_remap(pixel[1], 72f64);
            pixel[2] = u;
            pixel[3] = v;
        }
    }
}