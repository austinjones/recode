extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use video::video_format::*;
use video::video_frame::*;

// TODO: reduce pub usages
pub struct VideoBuffer {
    pub buffer: Vec<u8>,
    pub format: VideoFormat,
    pub time: f64,
    pub clock_time: gst::ClockTime,
    pub duration: gst::ClockTime
}

impl VideoBuffer {
    pub fn get_frame(&self, index: usize, format: &VideoFormat) -> &[u8] {
        let len = format.frame_size;
        let start = len*index;
        let end = len*(index+1);
        &self.buffer.as_slice()[(start as usize)..(end as usize)]
    }

    pub fn num_frames(&self) -> usize {
        self.buffer.len() / (self.format.frame_size as usize)
    }

    pub fn into_iter<'a>(self) -> VideoBufferIter {
        VideoBufferIter::new(self)
    }

    pub fn into_appsrc<'a>(self, appsrc: &'a mut gst_app::AppSrc) {

        // println!("Writing video buffer with time {:?} / duration {:?}", self.clock_time, self.duration);
        let mut buffer = gst::Buffer::with_size(self.buffer.len()).unwrap();
        {
            let buffer_ref = buffer.get_mut().unwrap();
            buffer_ref.set_pts(self.clock_time);
            buffer_ref.set_duration(self.duration);
            let mut buffer_writable = buffer_ref.map_writable().unwrap();
            let slice = buffer_writable.as_mut_slice();
            slice.copy_from_slice(self.buffer.as_slice());
        }

        let res = appsrc.push_buffer(buffer);
        if res != gst::FlowReturn::Ok {
            println!("Error writing Video Buffer: {:?}", res);
        }
    }
}

//  impl<'a> Iterator for AudioBufferIter<'a> {
//     type Item = AudioFrame;

// }

pub struct VideoBufferIter {
    buffer: VideoBuffer,
    pos: usize,
    window: usize
}

impl VideoBufferIter {
    pub fn new(buf: VideoBuffer) -> VideoBufferIter {
        VideoBufferIter {
            window: buf.format.frame_size,
            buffer: buf,
            pos: 0
        }
    }

    pub fn format(&self) -> VideoFormat {
        self.buffer.format
    }

    pub fn has_next(&self) -> bool {
        self.pos + self.window <= self.buffer.buffer.len()
    }

    pub fn next<'b>(&'b mut self) -> Option<VideoFrame<'b>> {
        if self.pos + self.window <= self.buffer.buffer.len() {
            let time = self.buffer.time + self.pos as f64 * self.buffer.format.frame_duration;

            let start = self.pos;
            let end = self.pos+self.window;

            let slice = &mut self.buffer.buffer.as_mut_slice()[start..end];
            let frame = VideoFrame::new(slice, &self.buffer.format, time);

            self.pos += 1;
            
            Some(frame)
        } else { None }
    }

    pub fn into_buffer(self) -> VideoBuffer {
        self.buffer
    }
 }

