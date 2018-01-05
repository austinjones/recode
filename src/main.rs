#![feature(conservative_impl_trait)]


mod audio;
mod pipeline;
mod video;

use pipeline::pipeline_utils::*;
use pipeline::frame_source::*;
use pipeline::frame_sink::*;

use pipeline::frame_transform::*;

/////////////

extern crate byteorder;

#[macro_use]
extern crate gstreamer;
extern crate gstreamer_app;
extern crate gstreamer_audio;
extern crate gstreamer_video;
extern crate byte_slice_cast;
extern crate rustfft;
extern crate num_complex;

use gstreamer::prelude::*;

use std::env;
#[cfg(feature = "v1_10")]
use std::sync::{RwLock, Mutex};


extern crate failure;
use failure::Error;

#[macro_use]
extern crate failure_derive;

fn example_main() -> Result<(), Error> {
    gstreamer::init()?;

    let args: Vec<_> = env::args().collect();
    let (uri, to): (&str, &str) = if args.len() == 3 {
        (args[1].as_ref(), args[2].as_ref())
    } else {
        println!("Usage: decodebin file_path");
        std::process::exit(-1)
    };
    
    println!("Creating framesource");
    let mut sink;
    {
        let (source, arx, vrx) = FrameSource::new(uri, to)?;
        println!("Spawning framesink");
        sink = FrameSink::spawn(to, FrameTransformImpl::new(), arx, vrx);
        println!("Running source pipeline...");
        // source.add_video_handler(|frame, timecode| {});
        // source.add_audio_handler(|sample, timecode| {});
        PipelineUtils::start(&source)?;
        PipelineUtils::message(&source)?;
        PipelineUtils::stop(&source)?;
    }


    println!("Done!  Waiting for sink pipeline to finish...");
    sink.join();

    println!("Done!");
    Ok(())
}

fn main() {
    match example_main() {
        Ok(_) => println!("Success!"),
        Err(e) => println!("Error! {}", e)
    }
}





// trait BorrowMutIterator<'a> {
//     type Item;
//     fn next(&'a mut self) -> Option<&'a mut Self::Item>;
// }



//  impl<'a> Iterator for AudioBufferIter<'a> {
//     type Item = AudioFrame;

// }



//  impl<'a> Iterator for VideoBufferIter<'a> {
//     type Item = VideoFrame<'a>;
//     fn next(&mut self) -> Option<VideoFrame<'a>> {
//         self.chunks.next().map(|e| VideoFrame::new(e))
//     }
// }

// impl<T> Iterator for AudioBuffer {
//     type Item = AudioFrame<'b>;
//     fn next(&mut self) -> Option<Self::Item> {
//         if self.pos < self.frames - 1 {
//             let len = self.format.frame_size;
//             let start = self.pos * len;
//             let end = (self.pos+1) * len;

//             self.pos += 1;

//             let slice = &mut self.buffer.as_mut_slice()[start..end];
//             let frame = AudioFrame::new(slice);
//             Some(&mut frame)
//         } else { None }

//     }
// }

// impl IntoIterator for AudioBuffer {
//     type Item=AudioFrame;
//     type IntoIter=std::;

//     fn into_iter(self) -> Self::IntoIter {
//         self.iter_mut()
//     }
// }



// pub struct Flatten<'a, I, J: 'a, K: 'a> {
//     iter: I,
//     front: Option<K>,
//     extend: &'a Fn(J) -> K
// }

// /// Create a new `Flatten` iterator.
// pub fn flatten<'a, I: IntoIterator<Item=J>, J, K, C: Fn(J) -> K>(iter: I, extend: C ) -> Flatten<'a, I, J, K> {
//     Flatten {
//         iter: iter,
//         front: None,
//         extend: &extend
//     }
// }

// impl<'a, I, J, K> Iterator for Flatten<'a, I, J, K>
//     where I: Iterator<Item=J>,
//           K: Iterator,
// {
//     type Item = K::Item;
//     fn next(&mut self) -> Option<Self::Item> {
//         loop {
//             if let Some(ref mut f) = self.front {
//                 match f.next() {
//                     elt @ Some(_) => return elt,
//                     None => { }
//                 }
//             }
//             if let Some(next_front) = self.iter.next() {
//                 self.front = Some((self.extend)(next_front).into_iter());
//             } else {
//                 break;
//             }
//         }
//         None
//     }
// }







// struct FrameIterator<B> {
//     reciever: Receiver<Arc<Mutex<B>>>,
//     buffer: Option<B>
// }

// impl<'b, B: BorrowMutIterator<'b> + 'b> FrameIterator<B>  {
//     fn new(reciever: Receiver<Arc<Mutex<B>>>) -> FrameIterator<B> {
//         FrameIterator {
//             reciever: reciever,
//             buffer: None
//         }
//     }

//     // pub fn current_buffer(&self) -> Option<B> {
//     //     self.buffer
//     // }

//     pub fn next_frame_in_buffer(&'b mut self) -> Option<B::Item> {
//         match self.buffer {
//             Some(ref mut buf) => buf.next(),
//             None => None
//         }
//     }

//     // pub fn next_buffer(&'b mut self) {
//     //     self.buffer = match self.reciever.recv().ok() {
//     //         // oh my god this is horrible... 
//     //         // I need to write a safe API for ownership transfer between arbitrary threads
//     //         // the problem with GStreamer is you have no control over the lifetime of the threads...
//     //         // so safe transfer requires Arc+Mutex
//     //         Some(arc) => Some(Mutex::into_inner(Arc::try_unwrap(arc).ok().unwrap()).ok().unwrap()),
//     //         None => None
//     //     };

//     //     self.buffer_iter = match self.buffer {
//     //         Some(ref mut buf) => {
//     //             let iterator = (self.iter_fn)(buf);
//     //             Some(&mut iterator)
//     //         },
//     //         None => None
//     //     };
//     // }

//     // pub fn next(&'b mut self) -> Option<F> {
//     //     match self.next_frame_in_buffer() {
//     //         Some(frame) => return Some(frame),
//     //         None => {}
//     //     }

//     //     self.next_buffer();
//     //     self.next_frame_in_buffer()
//     // }

//     fn finish_buffer(&self, b: &B) {
        
//     }

//     pub fn next(&'b mut self) -> Option<&'b mut B::Item> {
//         match self.buffer.iter_mut().flat_map(|e| e.next()).next() {
//             Some(frame) => return Some(frame),
//             None => {}
//         }


//         if let Some(ref buf) = self.buffer {
//             self.finish_buffer(buf);
//         }

//         self.buffer = match self.reciever.recv().ok() {
//             // oh my god this is horrible... 
//             // I need to write a safe API for ownership transfer between arbitrary threads
//             // the problem with GStreamer is you have no control over the lifetime of the threads...
//             // so safe transfer requires Arc+Mutex
//             Some(arc) => Some(Mutex::into_inner(Arc::try_unwrap(arc).ok().unwrap()).ok().unwrap()),
//             None => None
//         };

//         self.buffer.iter_mut().flat_map(|e| e.next()).next()
//     }
// }

// // impl<'b, B: AsMut<I>, F, I: 'b> Iterator for FrameIterator<'b, B, I> where I: Iterator<Item=F>{
// //     type Item=F;

// //     fn next(&mut self) -> Option<F> {
        
// //     }
// // }

