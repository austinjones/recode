use audio::audio_frame::*;
use video::video_frame::*;

use measures::*;

use pipeline::measures::*;

use stats::mean;
use stats::OnlineStats;

use pipeline::queue_buf::*;
use std::sync::Arc;

use rayon::prelude::*;
use rayon::*;
use rayon;

mod tests {
    use FrameTransformImpl;

    fn new_vec() -> Vec<f64> {
        vec![0f64, 0f64, 0f64, 1f64, 1f64, 1f64, 2f64, 2f64, 2f64]
    }

    fn new_asym() -> Vec<f64> {
        vec![0f64, 0f64, 0f64, 0f64, 1f64, 1f64, 1f64, 1f64, 2f64, 2f64, 2f64, 2f64]
    }

    #[test]
    pub fn test_linear_filter() {
        let mut vec = new_vec();
        FrameTransformImpl::box_filter(&mut vec, 1, 3, |cell, kernel| cell);
        assert_eq!(new_vec(), vec);
    }

    #[test]
    pub fn test_kernel() {
        let mut vec = new_vec();
        FrameTransformImpl::box_filter(&mut vec, 1, 3, |cell, kernel| kernel);
        let expected = vec![0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.5, 1.5, 1.5];
        assert_eq!(expected, vec);
    }

    #[test]
    pub fn test_asym() {
        let mut vec = new_asym();
        FrameTransformImpl::box_filter(&mut vec, 1, 4, |cell, kernel| kernel);
        let expected = vec![0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.5, 1.5, 1.5, 1.5];
        assert_eq!(expected, vec);
    }
}

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
    theta_r_buf: Option<QueueBuf<f64>>,
    r_buf: Option<QueueBuf<f64>>,
    fft: Option<FFTMeasure>,
    fft_map_cache: Option<Vec<Option<PixelMap>>>,
    angle: f64,
    premap: Premap
}

struct Premap {
    theta: Vec<f64>,
    magnitude: Vec<f64>,
    grayscale: Vec<f64>,
    y: Vec<f64>
}

impl Premap {

    pub fn new() -> Premap {
        let magnitude = Self::calculate_magnitudes();
        Premap {
            theta: Self::calculate_theta(),
            grayscale: Self::calculate_grayscale(&magnitude),
            magnitude: magnitude,
            y: Self::calculate_y()
        }
    }

    fn calculate_theta() -> Vec<f64> {
        (0..65536).map(|e| {
            let u = FrameTransformImpl::usize_to_f64(e / 256);
            let v = FrameTransformImpl::usize_to_f64(e % 256);

            (v).atan2(u)
        }).collect()
    }

    fn calculate_magnitudes() -> Vec<f64> {
        (0..65536).map(|e| {
            let u = FrameTransformImpl::usize_to_f64(e / 256);
            let v = FrameTransformImpl::usize_to_f64(e % 256);

            (u*u+v*v).sqrt()
        }).collect()
    }

    fn calculate_grayscale(magnitude_premap: &Vec<f64>) -> Vec<f64> {
        (0..65536).map(|e| {
            let zero_point = 100.0;
            let grayval = (128.0 - zero_point - magnitude_premap[e]).max(0.0) / (128.0 - zero_point);
            grayval.powf(0.35)
        }).collect()
    }
    
    fn calculate_y() -> Vec<f64> {
        (0..256).map(|y: usize| {
            // we remap the luminosity to increase the overall lightness of the image
            // this should be user input, not hardcoded
            // need to look into how video editors handle color maps...
            let y = FrameTransformImpl::sigmoid_remap(y as u8, 72f64);
            FrameTransformImpl::to_uf64(y)
        }).collect()
    }
}

impl FrameTransformImpl {
    pub fn new() -> FrameTransformImpl{ 
        FrameTransformImpl {
            frame_counter: 0,
            audio_edge: None,
            audio_volume: None,
            fft: None,
            fft_map_cache: None,
            theta_r_buf: None,
            r_buf: None,
            angle: 0f64,
            premap: Premap::new()
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

    fn bi_sigmoid(x: f64) -> f64 {
        1.0 - ::std::f64::consts::E.powf(-1f64 * x * x / 2.0)
    }

    fn remap(x: u8, min: u8, max:u8) -> u8 {
        let xf = x as f64;
        let df = (max-min) as f64;
        let outf = df * (xf / 255f64) + (min as f64);
        outf as u8
    }

    fn get_smooth(v: &Vec<f64>, idx: f64) -> f64 {
        let idx_vec = idx * (v.len() as f64 - 1.0);
        let lower = idx_vec.floor();
        let upper = idx_vec.ceil();
        
        if lower < 0.0 {
            return v[0];
        }

        if upper > (v.len() - 1) as f64 {
            return v[v.len() - 1];
        }

        let lower_val = v[lower as usize];
        let upper_val = v[upper as usize];

        let upper_ratio = idx_vec - lower;
        (1.0-upper_ratio) * lower_val + upper_ratio * upper_val
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

    pub fn box_filter<F>(vec: &mut Vec<f64>, box_radius: usize, width: usize, function: F) where F: Fn(f64, f64) -> f64 {
        let height = vec.len()/width;

        for vcell in 0..height {
            let vpos = vcell * width;

            let mut kernel = 0f64;
            let mut kernel_len = 0usize;
            for i in 0..box_radius+1 {
                kernel += vec[vpos + i];
                kernel_len += 1;
            }

            let mut output = vec!(0f64; width);
            for hcell in 0..width {
                // compute cell
                let cell_val = vec[vpos + hcell];
                let kernel_val = kernel / kernel_len as f64;
                output[hcell] = function(cell_val, kernel_val);
                // println!("H: {}x{}, vnew: {}, kernel: {}", vcell, hcell, vec[vpos+hcell], kernel);
                let add_pos = hcell + box_radius + 1;
                let remove_pos = hcell as isize - box_radius as isize;
                // println!("hcell: {}, box_radius: {}, remove_pos: {}", hcell, box_radius, remove_pos);
                if add_pos < width {
                    kernel += vec[vpos + add_pos];
                    kernel_len += 1;
                }
                if remove_pos >= 0 {
                    kernel -= vec[vpos + (remove_pos as usize)];
                    kernel_len -= 1;
                }
            }

            for hcell in 0..width {
                vec[vpos + hcell] = output[hcell];
            }
        }

        for hcell in 0..width {
            let mut kernel = 0f64;
            let mut kernel_len = 0usize;
            for i in 0..box_radius+1 {
                let vadd = vec[width * i + hcell];
                kernel += vadd;
                // println!("Vinit @ {} ... +{}", i, vadd);
                kernel_len += 1;
            }

            let mut output = vec!(0f64; height);
            for vcell in 0..height {
                let vpos = vcell * width;
                // comput cell
                let cell_val = vec[vpos + hcell];
                let kernel_val = kernel / kernel_len as f64;
                output[vcell] = function(cell_val, kernel_val);
                // println!("V: {}x{}, vnew: {}, kernel: {}", vcell, hcell, vec[vpos+hcell], kernel);

                let add_pos = vcell + box_radius + 1;
                let remove_pos = vcell as isize - box_radius as isize;
                if add_pos < height {
                    kernel += vec[add_pos * width + hcell];
                    // println!("Vadd @ {} ... {}", add_pos, kernel);
                    kernel_len += 1;
                }
                if remove_pos >= 0 {
                    let vec_val = vec[remove_pos as usize * width + hcell];
                    kernel -= vec[remove_pos as usize * width + hcell];
                    // println!("Vremove @ {} ... {} ... {}", remove_pos, kernel, vec_val);
                    kernel_len -= 1;
                }
            }
            
            for vcell in 0..height {
                let vpos = vcell * width;
                vec[vpos + hcell] = output[vcell];
            }
        }
    }

    fn box_blur(vec: &mut Vec<f64>, box_radius: f64, width: usize) {
        // this is a fast box blur algo.  we repeatedly apply horizontal and vertical line blur,
        // using a moving kernel.
        Self::box_filter(vec, Self::pixels_from(box_radius, width), width, |_, kernel| kernel);
    }

    fn box_edgefilter(vec: &mut Vec<f64>, box_radius: f64, width: usize, strength: f64) {
        // edge filter based on the linear blur filter.  
        // I think this will introduce linear artifacts during rotations,
        // but that might actually look cool.

        // It is also super super super fast.
        Self::box_filter(vec, Self::pixels_from(box_radius, width), width, |cell, kernel| {
            let diff = f64::abs(cell - kernel);
            cell + strength * diff
        });
    }

    fn pixels_from(scale: f64, width: usize) -> usize {
        usize::max(0, (scale * (width as f64)) as usize)
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

    fn to_f64(u: u8) -> f64 {
        (u as f64) - 128f64
    }

    fn to_uf64(u: u8) -> f64 {
        (u as f64)
    }

    fn usize_to_f64(u: usize) -> f64 {
        (u as f64) - 128f64
    }

    fn u_to_u8(u: f64) -> u8 {
        let n = u;
        if n < 0f64 {
            0u8
        } else if n > 255f64 {
            255u8
        } else {
            n.round() as u8
        }
    }

    fn to_u8(u: f64) -> u8 {
        let n = u + 128f64;
        if n < 0f64 {
            0u8
        } else if n > 255f64 {
            255u8
        } else {
            n.round() as u8
        }
    }

    fn init(&mut self, vframe: &VideoFrame) {
        if self.theta_r_buf.is_none() {
            let buf_size = vframe.format.frames_in(10.0);
            self.theta_r_buf = Some(QueueBuf::new(vec![0f64; buf_size]));
            self.r_buf = Some(QueueBuf::new(vec![1f64; buf_size]));
        }

        // let mut fft_output = vec!(Complex64::zero(); FFT_SIZE);
    }

    fn get_abs_vol(&mut self) -> f64 {
        let mut abs_vol = self.audio_volume.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if abs_vol.is_nan() {
            0f64
        } else {
            abs_vol
        }
    }

    fn get_disturbance(&mut self) -> f64{
        let mut disturbance = self.audio_edge.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if disturbance.is_nan() {
           0f64
        } else {
            disturbance
        }
    }

    fn update_angle(&mut self, rotation: f64, vframe: &VideoFrame) {
        self.angle += rotation * vframe.format.frame_duration;
        while self.angle > 1.0 {
            self.angle -= 1.0;
        }
    }

    fn calculate_theta_r(&mut self, vframe: &VideoFrame) -> (f64, f64) {
        let mut u_sum = 0f64;
        let mut v_sum = 0f64;
        let mut n_sum = 0usize;
        let scan_pixels = 257;
        for pixel in vframe.data.chunks(4 * scan_pixels) {
            let u = pixel[2];
            let v = pixel[3];
            let uv_idx = u as usize * 256 + v as usize;
            let theta = self.premap.theta[uv_idx];

            u_sum += theta.cos();
            v_sum += theta.sin();
            n_sum += 1;
        }

        let mut theta_r = v_sum.atan2(u_sum);
        while theta_r >= 2.0 * ::std::f64::consts::PI {
            theta_r -= 2.0 * ::std::f64::consts::PI;
        }
        while theta_r < 0.0 {
            theta_r += 2.0 * ::std::f64::consts::PI;
        }
        let sin_v = v_sum / n_sum as f64;
        let cos_u = u_sum / n_sum as f64;
        // println!("v_sum: {}, u_sum: {}, sin_v: {}, cos_u: {}", v_sum, u_sum, sin_v, cos_u);
        let r = (-(sin_v*sin_v + cos_u*cos_u).ln()).sqrt();
        println!("Theta_r: {:.2}, r: {:.2}", theta_r, r);
        self.theta_r_buf.as_mut().unwrap().push(theta_r);
        self.r_buf.as_mut().unwrap().push(r);
        let theta_r = mean(self.theta_r_buf.as_ref().unwrap().extract().iter().map(|e| *e));
        let r = mean(self.r_buf.as_ref().unwrap().extract().iter().map(|e| *e));
        println!("Avg theta-r: {:.2}, avg r: {:.2}", theta_r, r);
        (theta_r, r)
    }



    fn calculate_theta_framemap(&self, disturbance: f64, theta_r: f64, r: f64, abs_vol: f64) -> Vec<f64> {
            (0..65536).map(|e| {
            let u = Self::usize_to_f64(e / 256);
            let v = Self::usize_to_f64(e % 256);

            let pretheta = self.premap.theta[e]; // + 2f64 * ::std::f64::consts::PI * (self.angle + 0.05f64 * disturbance);
            
            let diff = pretheta - theta_r;

            let theta = diff / r;
            // println!("Theta: {:.2}, Pretheta: {:.2}, Theta_r: {:.2}, r: {:.2}", theta, pretheta, theta_r, r);
            // println!("Translated theta: {:.2}", theta);
            // PARAM: color spread
            let color_spread = 0.15 + 0.04 * abs_vol;
            let theta_1 = Self::sigmoid(theta);
            let theta_sig = 2f64 * color_spread * (theta_1 - 0.5f64);

            // PARAM: color spread
            // TODO: create crate for polar coordinates, or use an open source crate
            let theta_premap = self.angle + theta_sig + 0.05f64 * disturbance;
            // let theta_premap = disturbance;

            // oh my god the horror
            // I need to write a modular arethmitic type
            let mut theta_premap = 2f64 * ::std::f64::consts::PI * theta_premap;
            while theta_premap >= 2.0 * ::std::f64::consts::PI {
                theta_premap -= 2.0 * ::std::f64::consts::PI;
            }
            while theta_premap < 0.0 {
                theta_premap += 2.0 * ::std::f64::consts::PI;
            }

            let gray = self.premap.grayscale[e];
            let mut theta_premap = theta_premap + (theta_premap * 4.0).sin() / 4.0 + 3.0 * ::std::f64::consts::E.powf(-8.0 * (theta_premap - 0.8 * ::std::f64::consts::PI).powf(2.0));
            while theta_premap >= 2.0 * ::std::f64::consts::PI {
                theta_premap -= 2.0 * ::std::f64::consts::PI;
            }
            while theta_premap < 0.0 {
                theta_premap += 2.0 * ::std::f64::consts::PI;
            }
            // PARAM: gray basecolor
            let gray_offset = 1.0;
            let mut theta_premap = theta_premap + (1.0 - gray) * gray_offset * ::std::f64::consts::PI;
            while theta_premap >= 2.0 * ::std::f64::consts::PI {
                theta_premap -= 2.0 * ::std::f64::consts::PI;
            }
            while theta_premap < 0.0 {
                theta_premap += 2.0 * ::std::f64::consts::PI;
            }
            theta_premap
        }).collect()
    }

    fn calculate_saturation_framemap(&self, theta_framemap: &Vec<f64>, disturbance: f64, abs_vol: f64) -> Vec<f64> {
        (0..65536).map(|e| {
            let mut theta = theta_framemap[e];
            while theta > ::std::f64::consts::PI {
                theta -= 2f64 * ::std::f64::consts::PI;
            }
            
            // let fft_index = ((fft.len() - 1) as f64) * Self::bi_sigmoid(2f64 * theta);
            // let fft_floor = fft.get(fft_index.floor() as usize).map(|e| *e).unwrap_or(0f64);
            // let fft_ceil = fft.get(fft_index.ceil() as usize).map(|e| *e).unwrap_or(0f64);
            // let ceil_amount = fft_index - fft_index.floor();
            // let fft_val = ceil_amount * fft_ceil + (1f64 - ceil_amount) * fft_floor;
            // println!("Disturbance: {:.2}", disturbance);
            let gray_val = self.premap.grayscale[e];
            let base_saturation = 64.0 * (1.0 + abs_vol * 0.3);
            let gray_saturation = 90.0 * (1.0 + abs_vol * 0.3);
            gray_val * gray_saturation + (1.0-gray_val) * (base_saturation + 2f64 * self.premap.magnitude[e] + 16f64 * disturbance)
        }).collect()
    }

    fn calculate_uv_framemap(&self, magnitude_framemap: &Vec<f64>, theta_framemap: &Vec<f64>) -> Vec<(f64, f64)> {
        (0..65536).map(|e| {
            let mag = magnitude_framemap[e]; // * (1.5f64 + 1.1f64 * disturbance);
            let theta = theta_framemap[e];

            if theta.is_nan() || mag.is_nan() {
                return (0f64, 0f64);
            }

            let mut v = mag * theta.sin();
            let mut u = mag * theta.cos();

            // r could oversaturate colors.  it would move u/v outside the bounds of the u8 box
            // if this happens, to_u8 would truncate, changing the ratio of u:v
            // and thus changing the hue of the color.
            // we scale back the colors, so the truncation doesn't occur.
            if u > 127f64 {
                let scale = u / 127f64;
                v /= scale;
                u /= scale;
            }

            if v > 127f64 {
                let scale = v / 127f64;
                v /= scale;
                u /= scale;
            }

            if u < -128f64 {
                let scale = u / -128f64;
                v /= scale;
                u /= scale;
            }

            if v < -128f64 {
                let scale = v / -128f64;
                v /= scale;
                u /= scale;
            }

            // println!("Mag: {:.2}, Theta: {:.2}, u: {:.2}, v: {:.2}", mag, theta, u, v);

            (u, v)
        }).collect()
    }

    fn calculate_u_pixelmap(&self, vframe: &VideoFrame, uv_framemap: &Vec<(f64, f64)>) -> Vec<f64> {
        vframe.data.chunks(4).map(|pixel| {
            let u = pixel[2];
            let v = pixel[3];
            let uv_idx = u as usize * 256 + v as usize;
            let (u, _) = uv_framemap[uv_idx];
            u
        }).collect()
    }


    fn calculate_v_pixelmap(&self, vframe: &VideoFrame, uv_framemap: &Vec<(f64, f64)>) -> Vec<f64> {
        vframe.data.chunks(4).map(|pixel| {
            let u = pixel[2];
            let v = pixel[3];
            let uv_idx = u as usize * 256 + v as usize;
            let (_, v) = uv_framemap[uv_idx];
            v
        }).collect()
    }

    fn calculate_y_pixelmap(&self, vframe: &VideoFrame) -> Vec<f64> {
        vframe.data.chunks(4).map(|pixel| {
            self.premap.y[pixel[1] as usize]
        }).collect()
    }
}

struct PixelMap {
    idx: usize,
    x_pos: f64,
    y_pos: f64,
    scale_x: f64,
    scale_y: f64
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

        // if self.fft.is_none() {
        //     self.fft = Some(FFTMeasure::new(&aframe.format, 256));
        // }

        self.audio_edge.as_mut().unwrap().update(aframe);
        self.audio_volume.as_mut().unwrap().update(aframe);
        // self.fft.as_mut().unwrap().update(aframe);
    }

    fn process_video_frame(&mut self, vframe: &mut VideoFrame, vtime: f64) {
        self.init(vframe);
        let abs_vol = self.get_abs_vol();
        let disturbance = self.get_disturbance();
        let raw_rotation = (1f64+3f64*abs_vol) * ROTATION_RATE;
        self.update_angle(raw_rotation, vframe);
        
        println!("Raw Rotation: {:.2}, Angle: {:.2}, Time: {:.2}, Abs vol: {:.2}, audio_edge: {:.2}",raw_rotation, self.angle, vtime, abs_vol, disturbance);
        
        let (theta_r, r) = self.calculate_theta_r(vframe);

        let theta_framemap = self.calculate_theta_framemap(disturbance, abs_vol, theta_r, r);
        let saturation_framemap = self.calculate_saturation_framemap(&theta_framemap, disturbance, abs_vol);
        let uv_framemap = self.calculate_uv_framemap(&saturation_framemap, &theta_framemap);
        
        let mut y_pixelmap = None;
        let mut u_pixelmap = None;
        let mut v_pixelmap = None;

        // a bit of parallelism.
        // technically the y/u/v channels don't have any data dependency,
        // and the work done above is on small data sizes (65536 u/v colors)
        // this is the heavy lifting, so let's parallelize it.
        rayon::scope(|s| {
            s.spawn(|_| {
                let mut ys = self.calculate_y_pixelmap(vframe);
                Self::box_edgefilter(&mut ys, 0.0055, vframe.format.width as usize, 1.6);
                // gamma correction
                let ys: Vec<f64> = ys.iter().map(|y| {
                    0.68 * y.powf(1.05)
                }).collect();
                y_pixelmap = Some(ys);
            });

            s.spawn(|_| {
                let mut us = self.calculate_u_pixelmap(vframe, &uv_framemap);
                Self::box_blur(&mut us, 0.01, vframe.format.width as usize);
                u_pixelmap = Some(us);
            });

            s.spawn(|_| {
                let mut vs = self.calculate_v_pixelmap(vframe, &uv_framemap);
                Self::box_blur(&mut vs, 0.01, vframe.format.width as usize);
                v_pixelmap = Some(vs);
            });
        });

        // now collect the results and write them back into the frame
        let ys = y_pixelmap.unwrap();
        let us = u_pixelmap.unwrap();
        let vs = v_pixelmap.unwrap();

        let mut pixel_idx = 0usize;
        for pixel in vframe.data.chunks_mut(4) {
            pixel[1] = Self::u_to_u8(ys[pixel_idx]);
            pixel[2] = Self::to_u8(us[pixel_idx]);
            pixel[3] = Self::to_u8(vs[pixel_idx]);

            pixel_idx += 1;
        }
    }
}