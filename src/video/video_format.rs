extern crate gstreamer as gst;

#[derive(Copy, Clone, Debug)]
pub struct VideoFormat {
    pub frame_rate_gst_fraction: gst::Fraction,
    pub frame_rate: f64,
    pub frame_time: f64,
    pub width: i32,
    pub height: i32,
    pub pixel_count: i32,
    pub frame_size: usize,
    pub frame_duration: f64
}

impl VideoFormat {
    pub fn empty() -> VideoFormat {
        VideoFormat {
            frame_rate_gst_fraction: gst::Fraction::new(0, 1),
            frame_rate: 0f64,
            frame_time: 0f64,
            width: 0,
            height: 0,
            pixel_count: 0,
            frame_size: 0,
            frame_duration: 0f64
        }
    }

    pub fn new(framerate: gst::Fraction, width: i32, height: i32) -> VideoFormat {
        let (a, b): (i32, i32) = framerate.into();
        let rate = a as f64 / b as f64;
        VideoFormat {
            frame_rate_gst_fraction: framerate,
            frame_rate: rate,
            frame_time: 1f64 / rate,
            width: width, 
            height: height,
            pixel_count: width * height,
            frame_size: (4i32 * width * height) as usize,
            frame_duration: 1f64 / rate
        }
    }

    pub fn frames_in(&self, time: f64) -> usize {
        (time * (self.frame_rate as f64)).ceil() as usize
    }
}