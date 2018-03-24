use audio::audio_frame::*;
use video::video_frame::*;

use measures::*;

use pipeline::measures::*;

use stats::mean;
use stats::OnlineStats;

use pipeline::queue_buf::*;
use std::sync::Arc;

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
    angle: f64
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
                let add_pos = hcell + box_radius;
                let remove_pos = hcell as isize - box_radius as isize;
                if add_pos < width {
                    kernel += vec[vpos + add_pos];
                    kernel_len += 1;
                }
                if remove_pos >= 0 {
                    kernel -= vec[vpos + remove_pos as usize];
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

        if self.fft.is_none() {
            self.fft = Some(FFTMeasure::new(&aframe.format, 256));
        }

        self.audio_edge.as_mut().unwrap().update(aframe);
        self.audio_volume.as_mut().unwrap().update(aframe);
        // self.fft.as_mut().unwrap().update(aframe);
    }


    fn process_video_frame(&mut self, vframe: &mut VideoFrame, vtime: f64) {
        if self.theta_r_buf.is_none() {
            let buf_size = vframe.format.frames_in(10.0);
            self.theta_r_buf = Some(QueueBuf::new(vec![0f64; buf_size]));
            self.r_buf = Some(QueueBuf::new(vec![1f64; buf_size]));
        }

        // let mut fft_output = vec!(Complex64::zero(); FFT_SIZE);
        let mut abs_vol = self.audio_volume.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if abs_vol.is_nan() {
            abs_vol = 0f64;
        }

        // let fft: Vec<f64> = self.fft.as_mut().map(|e| e.value()).unwrap_or_else(|| vec!(0f64; 1));
        
        let mut disturbance = self.audio_edge.as_mut().map(|e| e.value(())).unwrap_or(0f64);
        if disturbance.is_nan() {
            disturbance = 0f64;
        }
        let raw_rotation = (1f64+3f64*abs_vol) * ROTATION_RATE;

        println!("Raw Rotation: {:.2}, Angle: {:.2}, Time: {:.2}, Abs vol: {:.2}, audio_edge: {:.2}",raw_rotation, self.angle, vtime, abs_vol, disturbance);
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
        while self.angle > 1.0 {
            self.angle -= 1.0;
        }
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

        let avg_range = 3isize;
        let avg_dispersion = 4isize;
        let avg_square = ((1 + 2 * avg_range) * (1 + 2 * avg_range)) as f64;
        let pixels = vframe.data.len() / 4usize;
        
        // let ys: Vec<f64> = vframe.data.chunks(4).map(|pixs| {
        //     pixs[1] as f64
        // }).collect();

        // let ys: Vec<f64> = (0..(ys.len())).into_iter().map(|pix_idx| {
        //     // println!("x: {}, y: {}", x_idx, y_idx);
        //     let mut sum = 0f64;

        //     for y_delta in -avg_range..avg_range {
        //         for x_delta in -avg_range..avg_range {
        //             let offset = (vframe.format.width as isize) * y_delta as isize + x_delta;
        //             let idx = (pix_idx as isize + offset) as usize;
        //             // println!("pix_idx: {}, x: {}, y: {}, x_delta: {}, y_delta: {}, offset: {}, idx: {}", pix_idx, x_idx, y_idx, x_delta, y_delta, offset, idx);
        //             if idx > 0 && ((idx as usize) < ys.len()) {
        //                 let y = ys[idx as usize];
        //                 sum += y;
        //             }
        //         }
        //     }

        //     sum / avg_square
        // }).collect();

        // let us: Vec<f64> = vframe.data.chunks(4).map(|pixs| {
        //     Self::to_f64(pixs[2])
        // }).collect();

        // TODO: experiment wich changing the calculation of idx here.
        // if idx is calculated with something other than vframe.format.width as the line size
        // some awesome interference patterns come out
        // maybe some sort of line-based remapping could be used as a filter for effects.
        // e.g. line width is 100 but things are mapped based on (80*y+x)
        // let us: Vec<f64> = (0..(us.len())).into_iter().map(|pix_idx| {
        //     // println!("x: {}, y: {}", x_idx, y_idx);
        //     let mut sum = 0f64;

        //     for y_delta in -avg_range..avg_range {
        //         for x_delta in -avg_range..avg_range {
        //             let offset = avg_dispersion * (vframe.format.width as isize) * y_delta as isize + avg_dispersion * x_delta;
        //             let idx = (pix_idx as isize + offset) as usize;
        //             // println!("pix_idx: {}, x: {}, y: {}, x_delta: {}, y_delta: {}, offset: {}, idx: {}", pix_idx, x_idx, y_idx, x_delta, y_delta, offset, idx);
        //             if idx > 0 && ((idx as usize) < us.len()) {
        //                 let u = us[idx as usize];
        //                 sum += u;
        //             }
        //         }
        //     }

        //     sum / avg_square
        // }).collect();

        // let vs: Vec<f64> = vframe.data.chunks(4).map(|pixs| {
        //     Self::to_f64(pixs[3])
        // }).collect();

        // let vs: Vec<f64> = (0..(vs.len())).into_iter().map(|pix_idx| {
        //     let mut sum = 0f64;

        //     for y_delta in -avg_range..avg_range {
        //         for x_delta in -avg_range..avg_range {
        //             let offset = avg_dispersion * (vframe.format.width as isize) * y_delta as isize + avg_dispersion * x_delta;
        //             let idx = (pix_idx as isize + offset) as usize;
        //             if idx > 0 && ((idx as usize) < us.len()) {
        //                 let v = vs[idx];
        //                 sum += v;
        //             }
        //         }
        //     }

        //     sum / avg_square
        // }).collect();

        // let thetas: Vec<f64> = us.iter().zip(vs.iter()).map(|(u,v)| {
        //     (v).atan2(*u)
        // }).collect();

        // let magnitudes: Vec<f64> = us.iter().zip(vs.iter()).map(|(u,v)| {
        //     (u*u+v*v).sqrt()
        // }).collect();

        let thetas: Vec<f64> = (0..65536).map(|e| {
            let u = Self::usize_to_f64(e / 256);
            let v = Self::usize_to_f64(e % 256);

            (v).atan2(u)
        }).collect();

        let magnitudes: Vec<f64> = (0..65536).map(|e| {
            let u = Self::usize_to_f64(e / 256);
            let v = Self::usize_to_f64(e % 256);

            (u*u+v*v).sqrt()
        }).collect();

        // let mut colorstats = OnlineStats::new();
        let mut u_sum = 0f64;
        let mut v_sum = 0f64;
        let mut n_sum = 0usize;
        let scan_pixels = 257;
        for pixel in vframe.data.chunks_mut(4 * scan_pixels) {
            let u = pixel[2];
            let v = pixel[3];
            let uv_idx = u as usize * 256 + v as usize;
            let theta = thetas[uv_idx];

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

        let gray_mags: Vec<f64> = (0..65536).map(|e| {
            let zero_point = 100.0;
            let grayval = (128.0 - zero_point - magnitudes[e]).max(0.0) / (128.0 - zero_point);
            grayval.powf(0.35)
        }).collect();

        let final_thetas: Vec<f64> = (0..65536).map(|e| {
            let u = Self::usize_to_f64(e / 256);
            let v = Self::usize_to_f64(e % 256);

            let pretheta = thetas[e]; // + 2f64 * ::std::f64::consts::PI * (self.angle + 0.05f64 * disturbance);
            
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
            let mut theta_premap = 2f64 * ::std::f64::consts::PI * theta_premap;
            while theta_premap >= 2.0 * ::std::f64::consts::PI {
                theta_premap -= 2.0 * ::std::f64::consts::PI;
            }
            while theta_premap < 0.0 {
                theta_premap += 2.0 * ::std::f64::consts::PI;
            }

            let gray = gray_mags[e];
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
        }).collect();

        let final_mags: Vec<f64> = (0..65536).map(|e| {
            let mut theta = final_thetas[e];
            while theta > ::std::f64::consts::PI {
                theta -= 2f64 * ::std::f64::consts::PI;
            }
            
            // let fft_index = ((fft.len() - 1) as f64) * Self::bi_sigmoid(2f64 * theta);
            // let fft_floor = fft.get(fft_index.floor() as usize).map(|e| *e).unwrap_or(0f64);
            // let fft_ceil = fft.get(fft_index.ceil() as usize).map(|e| *e).unwrap_or(0f64);
            // let ceil_amount = fft_index - fft_index.floor();
            // let fft_val = ceil_amount * fft_ceil + (1f64 - ceil_amount) * fft_floor;
            // println!("Disturbance: {:.2}", disturbance);
            let gray_val = gray_mags[e];
            let base_saturation = 64.0 * (1.0 + abs_vol * 0.3);
            let gray_saturation = 90.0 * (1.0 + abs_vol * 0.3);
            gray_val * gray_saturation + (1.0-gray_val) * (base_saturation + 2f64 * magnitudes[e] + 16f64 * disturbance)
        }).collect();

        let final_uv: Vec<(f64, f64)> = (0..65536).map(|e| {
            let mag = final_mags[e]; // * (1.5f64 + 1.1f64 * disturbance);
            let theta = final_thetas[e];

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
        }).collect();

        let final_y: Vec<u8> = (0..256).map(|y: usize| {
            // we remap the luminosity to increase the overall lightness of the image
            // this should be user input, not hardcoded
            // need to look into how video editors handle color maps...
            Self::sigmoid_remap(y as u8, 72f64)
        }).collect();

        // let thetas: Vec<f64> = (0..(thetas.len())).into_iter().map(|pix_idx| {
        //     let y_idx = pix_idx / (vframe.format.width as usize);
        //     let x_idx = pix_idx % (vframe.format.width as usize);
        //     let mut sin = 0f64;
        //     let mut cos = 0f64;

        //     for y_delta in -avg_range..avg_range {
        //         for x_delta in -avg_range..avg_range {
        //             let idx = 256isize * (y_idx as isize + y_delta) + x_idx as isize + x_delta;
        //             if idx > 0 && ((idx as usize) < thetas.len()) {
        //                 let t = thetas[idx as usize];
        //                 sin += t.sin();
        //                 cos += t.cos();
        //             }
        //         }
        //     }

        //     sin.atan2(cos)
        // }).collect();

        let mut us: Vec<f64> = vframe.data.chunks(4).map(|pixel| {
            let u = pixel[2];
            let v = pixel[3];
            let uv_idx = u as usize * 256 + v as usize;
            let (u, _) = final_uv[uv_idx];
            u
        }).collect();

        let mut vs: Vec<f64> = vframe.data.chunks(4).map(|pixel| {
            let preu = pixel[2];
            let prev = pixel[3];
            let uv_idx = preu as usize * 256 + prev as usize;
            let (_, v) = final_uv[uv_idx];
            // println!("V: {:.2} => {:.2}", prev, v);
            v
        }).collect();
        
        let mut ys: Vec<f64> = vframe.data.chunks(4).map(|pixel| {
            Self::to_uf64(final_y[pixel[1] as usize])
        }).collect();

        Self::box_blur(&mut us, 0.01, vframe.format.width as usize);
        Self::box_blur(&mut vs, 0.01, vframe.format.width as usize);
        Self::box_edgefilter(&mut ys, 0.0035, vframe.format.width as usize, 1.6);

        // gamma correction
        let mut ys: Vec<f64> = ys.iter().map(|y| {
            0.68 * y.powf(1.05)
        }).collect();

        // Self::box_edgefilter(&mut ys, 0.020, vframe.format.width as usize, 0.6);

        // if self.fft_map_cache.is_none() {
        //     let vec: Vec<Option<PixelMap>> = (0..vframe.format.pixel_count).map(|vdx| {
        //         let x = vdx % (vframe.format.width);
        //         let y = vdx / (vframe.format.width);

        //         let x_rel = x as f64 / vframe.format.width as f64;
        //         let y_rel = y as f64 / vframe.format.height as f64;

        //         let f_x = x_rel;
        //         let f_y = y_rel;
                
        //         // x(h-l) + l
        //         // (x + l) * (h - l) = i
        //         // (i - l) / (h-l)
        //         let aspect_correction = vframe.format.height as f64 / vframe.format.width as f64;
        //         let d_x = f_x - 0.5;
        //         let d_y = f_y - 0.5;
        //         let d_z = 0.37;
        //         let P = (d_x*d_x + d_y*d_y + d_z*d_z).sqrt();
        //         let f_x = 0.5 + d_x * P.abs();
        //         let f_y = 0.5 + d_y * P.abs();

        //         let x_min = 0.185;
        //         let x_max = 0.808;

        //         let y_min = 0.190;
        //         let y_max = 0.808;

        //         let f_x = (f_x - x_min) / (x_max - x_min);
        //         let f_y = (f_y - y_min) / (y_max - y_min);

        //         if f_x < 0f64 || f_x > 1f64 || f_y < 0f64 || f_y > 1f64 {
        //             // we want to map some pixels outside of the bounding box
        //             // these are pixels outside of the fisheye
        //             // for these pixels, we return None, since we don't want to map any color.
        //             return None;
        //         }

        //         let f_x = 1.0 - (2.0 * (f_x - 0.5).abs());
        //         let f_y = 1.0 - (2.0 * (f_y - 0.5).abs());
        //         // println!("fft_x: {:.1}, fft_y: {:.1}", fft_x, fft_y);

        //         // was -25, +13
        //         let x_sin = (vdx % (vframe.format.width)) as f64 / vframe.format.width as f64;
        //         let y_sin = (vdx / (vframe.format.width)) as f64 / vframe.format.height as f64;
        //         // println!("x: {:.2},  y: {:.2}, x_sin: {:.2}, y_sin: {:.2}", x_rel, y_rel, x_sin, y_sin);

        //         let sin_scale = 150.0;
        //         let x_comp = (f_x as f64 * sin_scale).sin();
        //         let y_comp = (f_y as f64 * sin_scale).cos();

        //         let edge_fade = 0.01;
        //         let edge_dist_x = f_x.min(1.0-f_x);
        //         let edge_dist_y = f_y.min(1.0-f_y);
        //         let scale = ((1.0/edge_fade) * edge_dist_x.min(edge_dist_y)).min(1.0);

        //         let map = PixelMap {
        //             idx: vdx as usize,
        //             x_pos: f_x,
        //             y_pos: f_y,
        //             scale_x: scale * x_comp,
        //             scale_y: scale * y_comp
        //         };
        //         Some(map)
        //     }).filter(|e| e.is_some())
        //       .collect();
        //     self.fft_map_cache = Some(vec);
        // }

        // for map in self.fft_map_cache.as_ref().unwrap().iter() {
        //     if map.is_none() {
        //         continue;
        //     }

        //     let map = map.as_ref().unwrap();

        //     let v = vs[map.idx];
        //     let u = us[map.idx];

        //     let fft_val_x = Self::get_smooth(&fft, map.x_pos);
        //     let fft_val_y = Self::get_smooth(&fft, map.y_pos);

        //     // there is a sin wave transformation hidden in scale_x and scale_y
        //     let mutation = fft_val_x * map.scale_x + fft_val_y * map.scale_y;
        //     vs[map.idx] = v * (1.0f64 + 0.75f64 * mutation);
        //     us[map.idx] = u * (1.0f64 + 0.75f64 * mutation);
        // }

        // for (v, u) in vs.iter_mut().zip(us.iter_mut()) {
        //     let x = vdx % (vframe.format.width);
        //     let y = vdx / (vframe.format.width);

        //     let x_rel = x as f64 / vframe.format.width as f64;
        //     let y_rel = y as f64 / vframe.format.height as f64;

        //     let f_x = x_rel;
        //     let f_y = y_rel;
            
        //     // x(h-l) + l
        //     // (x + l) * (h - l) = i
        //     // (i - l) / (h-l)
        //     let d_x = f_x - 0.5;
        //     let d_y = f_y - 0.5;
        //     let d_z = 0.39;
        //     let P = (d_x*d_x + d_y*d_y + d_z*d_z).sqrt();
        //     let f_x = 0.5 + d_x * P.abs();
        //     let f_y = 0.5 + d_y * P.abs();

        //     let x_min = 0.183;
        //     let x_max = 0.811;

        //     let y_min = 0.185;
        //     let y_max = 0.813;

        //     let f_x = (f_x - x_min) / (x_max - x_min);
        //     let f_y = (f_y - y_min) / (y_max - y_min);

        //     if f_x < 0f64 || f_x > 1f64 || f_y < 0f64 || f_y > 1f64 {
        //         if vdx % 1000000 == 0 {
        //              println!("x_rel: {:.2}, f_x: {:.2}, y_rel: {:.2}, f_y: {:.2}", x_rel, f_x, y_rel, f_y);
        //         }
        //         vdx += 1;
        //         continue;
        //     } else {
                
        //     }


        //     let fft_scale = 1.0 / 3.0;
        //     let sin_scale = 250.0;

        //     let f_x = 1.0 - (2.0 * (f_x - 0.5).abs());
        //     let f_y = 1.0 - (2.0 * (f_y - 0.5).abs());
        //     // println!("fft_x: {:.1}, fft_y: {:.1}", fft_x, fft_y);
        //     let fft_val_x = Self::get_smooth(&fft, f_x);
        //     let fft_val_y = Self::get_smooth(&fft, f_y);

        //     // was -25, +13
        //     let x_sin = (vdx % (vframe.format.width)) as f64 / vframe.format.width as f64;
        //     let y_sin = (vdx / (vframe.format.width)) as f64 / vframe.format.height as f64;
        //     // println!("x: {:.2},  y: {:.2}, x_sin: {:.2}, y_sin: {:.2}", x_rel, y_rel, x_sin, y_sin);
        //     let x_comp = (f_x as f64 * sin_scale).sin();
        //     let y_comp = (f_y as f64 * sin_scale).cos();

        //     let edge_fade = 0.01;
        //     let edge_dist_x = f_x.min(1.0-f_x);
        //     let edge_dist_y = f_y.min(1.0-f_y);
        //     let scale = ((1.0/edge_fade) * edge_dist_x.min(edge_dist_y)).min(1.0);

        //     let strength = scale * 0.75f64;
        //     let mutation = fft_val_x * x_comp + fft_val_y * y_comp;

        //     *v = *v * (1.0f64 + strength * mutation); // + strength * fft_val_y;
        //     *u = *v * (1.0f64 + strength * mutation); // + strength * fft_val_y;
        //     vdx += 1;
        // }

        // for v in vs.iter_mut() {
        //     let x = vdx % (vframe.format.width - 25);
        //     let y = vdx / (vframe.format.width + 13);
        //     let x_comp = 12f64 * (x as f64 / 5f64).sin();
        //     let y_comp = 12f64 * (y as f64 / 5f64).sin();
        //     *v = *v + x_comp + y_comp;
        // }
        // for v in vs.iter_mut() {
        //     let x = vdx % (vframe.format.width - 25);
        //     let y = vdx / (vframe.format.width + 13);
        //     let x_comp = 12f64 * (x as f64 / 5f64).sin();
        //     let y_comp = 12f64 * (y as f64 / 5f64).sin();
        //     *v = *v + x_comp + y_comp;
        //     vdx += 1;
        // }

        let mut pixel_idx = 0usize;
        for pixel in vframe.data.chunks_mut(4) {
            pixel[1] = Self::u_to_u8(ys[pixel_idx]);
            pixel[2] = Self::to_u8(us[pixel_idx]);
            pixel[3] = Self::to_u8(vs[pixel_idx]);

            pixel_idx += 1;
        }
        

        // let x_size = vframe.format.width as usize;
        // let y_size = vframe.format.height as usize;

        // let smooth_dist = 2;
        // let smooth_area = ((2*smooth_dist+1) * (2*smooth_dist+1)) as usize;
        // let avg_dispersion = 4;
        // let pixel_len = x_size * y_size;
        // for pixel_idx in 0..pixel_len {
        //     let pixel_y = pixel_idx / x_size;
        //     let mut us = 0f64;
        //     let mut vs = 0f64;
        //     for y_delta in -smooth_dist..(smooth_dist+1) {
        //         for x_delta in -smooth_dist..(smooth_dist+1) {
        //             let offset = avg_dispersion * (vframe.format.width as isize) * y_delta as isize + avg_dispersion * x_delta;
        //             let idx = (pixel_idx as isize + offset) as usize;

        //             // x delta might slip over a line boundary
        //             // this prevents pixels at the beginning/end of the adjacent lines from being included
        //             let offset_y = (pixel_y as isize + avg_dispersion * y_delta) as usize;
        //             let actual_y = idx / x_size;
        //             // println!("pix_idx: {}, x_delta: {}, y_delta: {}, offset: {}, idx: {}", pixel_idx, x_delta, y_delta, offset, idx);
        //             // println!("offset_y: {}, actual_y: {}", offset_y, actual_y);
        //             if idx > 0 && ((idx as usize) < pixel_len) && offset_y == actual_y {
        //                 us += Self::to_f64(vframe.data[4*idx+2]);
        //                 vs += Self::to_f64(vframe.data[4*idx+3]);
        //             }
        //         }
        //     }

        //     let u_final = us / smooth_area as f64;
        //     let v_final = vs / smooth_area as f64;

        //     vframe.data[4*pixel_idx+2] = Self::to_u8(u_final);
        //     vframe.data[4*pixel_idx+3] = Self::to_u8(v_final);
        // }
    }
}