use std::collections::LinkedList;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use audio::audio_buffer::*;
use audio::audio_format::*;
use audio::audio_frame::*;

pub struct AudioIter {
    audio_channel: Box<Iterator<Item=AudioBuffer>>,
    audio_frame_iterator: Option<AudioBufferIter>,
    finished_buffers: LinkedList<AudioBuffer>
}

impl AudioIter {
    pub fn new(audio_channel: Receiver<Arc<Mutex<AudioBuffer>>>) -> AudioIter {
        let audio_iter = audio_channel.into_iter().map(|e| Mutex::into_inner(Arc::try_unwrap(e).ok().unwrap()).ok().unwrap());
        let mut processor = AudioIter {
            audio_frame_iterator: None,
            audio_channel: Box::new(audio_iter),
            finished_buffers: LinkedList::new()
        };

        processor.next_audio_buffer();

        processor
    }

    pub fn format(&mut self) -> Option<AudioFormat> {
        if self.audio_frame_iterator.is_none() {
            self.next_audio_buffer();
        }

        match &self.audio_frame_iterator {
            &Some(ref iter) => Some(iter.format()),
            &None => None
        }
    }

    pub fn next_finished_buffer(&mut self) -> Option<AudioBuffer> {
        self.finished_buffers.pop_back()
    }

    fn next_audio_buffer(&mut self) {
        let last_iter = ::std::mem::replace(&mut self.audio_frame_iterator, None);
        match last_iter {
            Some(iter) => self.finished_buffers.push_front(iter.into_buffer()),
            _ => {}
        }

        self.audio_frame_iterator = match self.audio_channel.next() {
            Some(buf) => Some(buf.into_iter()),
            None => None
        };
    }

    fn next_audio_frame_in_buffer(&mut self) -> Option<AudioFrame> {
       if let Some(ref mut iter) = self.audio_frame_iterator {
            iter.next()
        } else {
            None
        }
    }

    pub fn next_audio_frame(&mut self) -> Option<AudioFrame> {
        if self.audio_frame_iterator.is_some() && self.audio_frame_iterator.as_ref().unwrap().has_next() {
            return self.next_audio_frame_in_buffer();
        }

        self.next_audio_buffer();
        self.next_audio_frame_in_buffer()
    }
}