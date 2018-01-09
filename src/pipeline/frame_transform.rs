use audio::audio_frame::*;
use video::video_frame::*;

use measures::*;

use pipeline::measures::*;



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
    frame_counter: usize,
    audio_edge: Option<NormalizedAudioEdgeMeasure>,
    audio_volume: Option<NormalizedAudioVolumeMeasure>,
    fft: Option<FFTMeasure>,
    angle: f64
}

impl FrameTransformImpl {
    pub fn new() -> FrameTransformImpl{ 
        FrameTransformImpl {
            frame_counter: 0,
            audio_edge: None,
            audio_volume: None,
            fft: None,
            angle: 0f64
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
        // if self.frame_counter < 50 {
        //     self.frame_counter += 1;
        //     return;
        // } else {
        //     self.frame_counter = 0;
        // }
        if aframe.sum() == 0f64 {
            return;
        }

        if self.audio_edge.is_none() {
            self.audio_edge = Some(NormalizedAudioEdgeMeasure::new(&aframe.format));
        }

        if self.audio_volume.is_none() {
            self.audio_volume = Some(NormalizedAudioVolumeMeasure::new(&aframe.format));
        }

        if self.fft.is_none() {
            self.fft = Some(FFTMeasure::new(&aframe.format, 256));
        }

        self.audio_edge.as_mut().unwrap().update(aframe);
        self.audio_volume.as_mut().unwrap().update(aframe);
        self.fft.as_mut().unwrap().update(aframe);
    }


    fn process_video_frame(&mut self, vframe: &mut VideoFrame, vtime: f64) {
        // let mut fft_output = vec!(Complex64::zero(); FFT_SIZE);
        let mut abs_vol = self.audio_volume.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if abs_vol.is_nan() {
            abs_vol = 0f64;
        }

        let fft: Vec<f64> = self.fft.as_mut().map(|e| e.value()).unwrap_or_else(|| vec!(0f64; 256));
        
        let mut disturbance = self.audio_edge.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if disturbance.is_nan() {
            disturbance = 0f64;
        }
        let raw_rotation = (1f64+3f64*abs_vol) * ROTATION_RATE;

        println!("Time: {:.2}, Abs vol: {:.2}, audio_edge: {:.2}", vtime, abs_vol, disturbance);
        // self.fft.process(audio_vec.as_mut_slice(), fft_output.as_mut_slice());
        // let fft_output: Vec<f64> = fft_output[0..FFT_SIZE/2].iter().map(|e| e.norm_sqr()).collect();
        // let center_of_mass = Self::center_of_mass(fft_output.as_slice());
        // let avg = fft_output.iter().fold(0f64, |a,b| a+b) / (fft_output.len() as f64);
        // let fft_output: Vec<f64> = fft_output.iter().map(|e| e / avg).collect();

        // let low = fft_output[0..FFT_SIZE/4].iter().fold(0f64, |a,b| a+b) / (FFT_SIZE as f64 / 2f64);
        // let high = fft_output[FFT_SIZE/4..FFT_SIZE/2].iter().fold(0f64, |a,b| a+b) / (FFT_SIZE as f64 / 2f64);
        // println!("Low: {}, High: {}", low, high);

        // let rotation = (vtime % 7f64) / 7f64;
        // println!("Avg vol: {}, Max Vol: {}, vol ratio: {}, avg ratio: {}", avg_vol, self.max_vol, abs_vol, avg_abs_vol);
        self.angle += raw_rotation * vframe.format.frame_duration;

        // self.disturb_buf.push(raw_disturb);
        // println!("Vol_Decay: {}, disturb: {}", self.volratio_decay, disturbance);
        // let disturbance = Self::avg(self.disturb_buf.extract().as_slice());
        // let disturbance = Self::sigmoid(5f64*disturbance)-0.5f64;
        // let disturbance = Self::sigmoid(5f64*(self.abs_vol_decay - avg_abs_vol))-0.5f64;
        // println!("Rotation: {}, raw disturb: {}, disturbance: {}", raw_rotation, raw_disturb, disturbance);
        // let rotation = Self::sigmoid(15f64 * (high - low))-0.5f64);

        
        // println!("Low: {}, High: {}", low, high);
        // println!("Diff: {}, Rotation: {}", (high-low), rotation);

        // if self.queue_buf.is_saturated() {
        //     // println!("FFT input: {:?}", audio_vec);
        //     // println!("FFT output: {:?}", fft_output);
        // }

        for pixel in vframe.data.chunks_mut(4) {
            let fft_index = 256 - (pixel[1] as usize);
            let fft_val = fft.get(fft_index).map(|e| *e).unwrap_or(0f64);

            let (u, v) = Self::rotate(pixel[1], pixel[2], pixel[3], 
                self.angle + (0.05f64 * disturbance), 
                1.5f64 + 1.1f64 * disturbance);
            // pixel[2] = pixel[2].wrapping_add(rotation);
            
            pixel[1] = Self::sigmoid_remap(pixel[1], 72f64);
            pixel[2] = u;
            pixel[3] = v;
        }
    }
}