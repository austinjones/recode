use video::video_format::*;

pub struct VideoFrame<'a> {
    pub data: &'a mut [u8],
    pub format: &'a VideoFormat,
    pub time: f64
}

impl<'a> VideoFrame<'a> {
    pub fn new(d: &'a mut [u8], format: &'a VideoFormat, time: f64) -> VideoFrame<'a> {
        VideoFrame {
            data: d,
            format: format,
            time: time
        }
    }
}