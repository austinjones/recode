
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};
use gstreamer;
use gstreamer_app;

use audio::audio_format::*;
use audio::audio_frame::*;

pub struct AudioBuffer {
    pos: usize,
    frames: usize,
    buffer: Vec<i32>,
    pub format: AudioFormat,
    pub time: f64,
    clock_time: gstreamer::ClockTime,
    duration: gstreamer::ClockTime
}

impl AudioBuffer {
    pub fn new(buffer: Vec<i32>, format: AudioFormat, clock_time: gstreamer::ClockTime, duration: gstreamer::ClockTime) -> AudioBuffer {
        AudioBuffer {
            pos: 0,
            frames: buffer.len() / format.frame_size,
            buffer: buffer, 
            format: format,
            time: clock_time.nanoseconds().unwrap_or(0u64) as f64 / 1_000_000_000f64,
            clock_time: clock_time,
            duration: duration
        }
    }

    pub fn num_frames(&self, format: &AudioFormat) -> usize {
        self.buffer.len() / (format.channels as usize)
    }

    // pub fn iter<'a>(&'a self) -> Chunks<'a, i32> {
    //     self.buffer.chunks(self.format.frame_size)
    // }

    pub fn into_iter<'a>(self) -> AudioBufferIter {
        AudioBufferIter::new(self)
    }

    pub fn into_appsrc<'a>(self, appsrc: &'a mut gstreamer_app::AppSrc) {
        // println!("Writing audio buffer with time {:?} / duration {:?}", self.clock_time, self.duration);
        let i32_size = ::std::mem::size_of::<i32>();
        let mut buffer = gstreamer::Buffer::with_size(self.buffer.len() * i32_size).unwrap();
        {

            let buffer_ref = buffer.get_mut().unwrap();           
            buffer_ref.set_pts(self.clock_time);
            buffer_ref.set_duration(self.duration);
            let mut buffer_writable = buffer_ref.map_writable().unwrap();
            let mut slice = buffer_writable.as_mut_slice();
            for int in self.buffer {
                slice.write_i32::<LittleEndian>(int);
            }
        }
        // println!("Pushing audio buffer into appsink");
        if appsrc.push_buffer(buffer) != gstreamer::FlowReturn::Ok {
            println!("Error writing Audio Buffer")
        }
    }
}

pub struct AudioBufferIter {
    buffer: AudioBuffer,
    pos: usize,
    window: usize
}

impl AudioBufferIter {
    pub fn new(buf: AudioBuffer) -> AudioBufferIter {
        AudioBufferIter {
            window: buf.format.frame_size,
            buffer: buf,
            pos: 0
        }
    }

    pub fn format(&self) -> AudioFormat {
        self.buffer.format
    }

    pub fn has_next(&self) -> bool {
        self.pos + self.window <= self.buffer.buffer.len()
    }

    pub fn next<'b>(&'b mut self) -> Option<AudioFrame<'b>> {
        if self.pos + self.window <= self.buffer.buffer.len() {
            let time = self.buffer.time + self.pos as f64 * self.buffer.format.frame_duration;

            let start = self.pos;
            let end = self.pos+self.window;

            let slice = &mut self.buffer.buffer.as_mut_slice()[start..end];
            let frame = AudioFrame::new(slice, &self.buffer.format, time);

            self.pos += 1;

            Some(frame)
        } else { None }
    }

    pub fn into_buffer(self) -> AudioBuffer {
        self.buffer
    }
 }