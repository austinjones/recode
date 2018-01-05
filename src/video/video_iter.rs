use std::collections::LinkedList;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use video::video_buffer::*;
use video::video_format::*;
use video::video_frame::*;

pub struct VideoIter {
    video_channel: Box<Iterator<Item=VideoBuffer>>,
    video_frame_iterator: Option<VideoBufferIter>,
    finished_buffers: LinkedList<VideoBuffer>
}

impl<'i> VideoIter {
    pub fn new(video_channel: Receiver<Arc<Mutex<VideoBuffer>>>) -> VideoIter {
        let video_iter = video_channel.into_iter().map(|e| Mutex::into_inner(Arc::try_unwrap(e).ok().unwrap()).ok().unwrap());
        let mut processor = VideoIter {
            video_frame_iterator: None,
            video_channel: Box::new(video_iter),
            finished_buffers: LinkedList::new()
        };

        processor.next_video_buffer();

        processor
    }

    pub fn format(&mut self) -> Option<VideoFormat> {
        if self.video_frame_iterator.is_none() {
            self.next_video_buffer();
        }

        match &self.video_frame_iterator {
            &Some(ref iter) => Some(iter.format()),
            &None => None
        }
    }

    pub fn next_finished_buffer(&mut self) -> Option<VideoBuffer> {
        self.finished_buffers.pop_back()
    }

    fn next_video_buffer(&mut self) {
        let last_iter = ::std::mem::replace(&mut self.video_frame_iterator, None);
        match last_iter {
            Some(iter) => self.finished_buffers.push_front(iter.into_buffer()),
            _ => {}
        }

        self.video_frame_iterator = match self.video_channel.next() {
            Some(buf) => Some(buf.into_iter()),
            None => None
        };
    }

    fn next_video_frame_in_buffer(&mut self) -> Option<VideoFrame> {
        if let Some(ref mut iter) = self.video_frame_iterator {
            iter.next()
        } else {
            None
        }
    }

    pub fn next_video_frame(&mut self) -> Option<VideoFrame> {
        if self.video_frame_iterator.is_some() && self.video_frame_iterator.as_ref().unwrap().has_next() {
            return self.next_video_frame_in_buffer();
        }

        self.next_video_buffer();
        self.next_video_frame_in_buffer()
    }
}