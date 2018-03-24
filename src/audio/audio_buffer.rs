
use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};
use gstreamer;
use gstreamer_app;

use audio::audio_format::*;
use audio::audio_frame::*;

pub struct AudioBuffer {
    pos: usize,
    frames: usize,
    buffer: gstreamer::Buffer,
    samples: Vec<i16>,
    pub format: AudioFormat,
    pub time: f64
}

impl AudioBuffer {
    pub fn new(buffer: gstreamer::Buffer, samples: Vec<i16>, format: AudioFormat) -> AudioBuffer {
        AudioBuffer {
            time: buffer.get_pts().nanoseconds().unwrap_or(0u64) as f64 / 1_000_000_000f64,
            pos: 0,
            frames: samples.len() / format.frame_size,
            buffer: buffer, 
            samples: samples,
            format: format
        }
    }

    pub fn num_frames(&self, format: &AudioFormat) -> usize {
        self.samples.len() / (format.channels as usize)
    }

    // pub fn iter<'a>(&'a self) -> Chunks<'a, i32> {
    //     self.buffer.chunks(self.format.frame_size)
    // }

    pub fn into_iter<'a>(self) -> AudioBufferIter {
        AudioBufferIter::new(self)
    }

    pub fn into_appsrc<'a>(self, appsrc: &'a mut gstreamer_app::AppSrc) {
        // println!("Writing audio buffer with time {:?} / duration {:?}", self.clock_time, self.duration);
        // let i32_size = ::std::mem::size_of::<i32>();
        // let buf_size = self.buffer.len() * i32_size;
        // println!("Assuming output size: {}", buf_size);
        // let mut buffer = gstreamer::Buffer::with_size(buf_size).unwrap();

        // // for f in self.buffer.chunks(2) {
        // //     println!("LEFT: {:?}", f[0]);
        // // }

        // // for f in self.buffer.chunks(2) {
        // //     println!("RIGHT: {:?}", f[1]);
        // // }

        // {
        //     let buffer_ref = buffer.get_mut().unwrap();           
        //     buffer_ref.set_pts(self.pts); 
        //     buffer_ref.set_dts(self.dts);
        //     buffer_ref.set_duration(self.duration);
        //     buffer_ref.set_offset(self.offset);
        //     let mut buffer_writable = buffer_ref.map_writable().unwrap();
        //     let mut slice = buffer_writable.as_mut_slice();
        //     LittleEndian::write_i32_into(self.buffer.as_slice(), slice);
        //     // for f in slice.chunks(i32_size) {
        //     //     println!("Output byte: {:?}", f);
        //     // }
        // }
        // println!("Pushing audio buffer into appsink");
        println!("OUT {}", self.buffer.get_pts());
        if appsrc.push_buffer(self.buffer) != gstreamer::FlowReturn::Ok {
            println!("Error writing Audio Buffer")
        }
    }
}

pub struct AudioBufferIter {
    buffer: AudioBuffer,
    pos: usize,
    window: usize
}

impl<'i> AudioBufferIter {
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
        self.pos + self.window <= self.buffer.samples.len()
    }

    pub fn next<'b>(&'b mut self) -> Option<AudioFrame<'b>> {
        if self.pos + self.window <= self.buffer.samples.len() {
            let time = self.buffer.time + self.pos as f64 * self.buffer.format.frame_duration;

            let start = self.pos;
            let end = self.pos+self.window;

            let slice = &self.buffer.samples[start..end];
            let frame = AudioFrame::new(slice, &self.buffer.format, time);

            self.pos += 1;

            Some(frame)
        } else { None }
    }

    pub fn into_buffer(self) -> AudioBuffer {
        self.buffer
    }
 }