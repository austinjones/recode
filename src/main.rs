#![feature(conservative_impl_trait)]


mod audio;
mod pipeline;
mod video;
mod measures;
mod osx;

use std::thread;

use pipeline::pipeline_utils::*;
use pipeline::frame_source::*;
use pipeline::frame_sink::*;

use pipeline::frame_transform::*;
use osx::*;

/////////////

extern crate byteorder;

#[macro_use]
extern crate gstreamer;
extern crate gstreamer_app;
extern crate gstreamer_audio;
extern crate gstreamer_video;
extern crate byte_slice_cast;
extern crate rustfft;
extern crate apodize;
extern crate num_complex;
extern crate stats;
extern crate glib;

extern crate cpuprofiler;

#[macro_use]
extern crate serde_derive;
extern crate docopt;

use docopt::Docopt;

use gstreamer::prelude::*;

use std::env;
#[cfg(feature = "v1_10")]
use std::sync::{RwLock, Mutex};

extern crate failure;
use failure::Error;

#[macro_use]
extern crate failure_derive;

const USAGE: &'static str = "
Recode.

Usage:
  recode convert <input-mp4> <output-mp4>
  recode preview <input-mp4>
  recode trace <input-mp4> <measure>
  recode (-h | --help)
  recode --version

Options:
  -h --help     Show this screen.
  --version     Show version.
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_input_mp4: String,
    arg_output_mp4: String,
    arg_measure: String,
    cmd_convert: bool,
    cmd_preview: bool,
    cmd_trace: bool
}

impl Args {
    fn get_sinktype(&self) -> Option<SinkType> {
        if self.cmd_preview {
            Some(SinkType::playback)
        } else if self.cmd_convert {
            Some(SinkType::file_mp4(self.arg_output_mp4.clone()))
        } else {
            None
        }
    }
}

fn example_main() -> Result<(), Error> {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());

    gstreamer::init()?;


    let sinktype = args.get_sinktype();
    if sinktype.is_some() {
        let sinktype = sinktype.unwrap();
        let uri_from = args.arg_input_mp4.as_str();
        let uri_to = args.arg_output_mp4.as_str();

        println!("Creating framesource");
        let mut sink;
        {
            let (source, arx, vrx) = FrameSource::new(uri_from)?;
            println!("Spawning framesink");
            sink = FrameSink::spawn(sinktype, FrameTransformImpl::new(), arx, vrx);
            println!("Running source pipeline...");
            // source.add_video_handler(|frame, timecode| {});
            // source.add_audio_handler(|sample, timecode| {});
            PipelineUtils::start(&source)?;
            PipelineUtils::message(&source)?;
            PipelineUtils::stop(&source)?;
        }

        println!("Done!  Waiting for sink pipeline to finish...");
        sink.join();
    } else {
        println!("Unknown command!");
    }

    

    
    println!("Done!");
    Ok(())
}

fn main() {
    let main_loop = glib::MainLoop::new(glib::MainContext::default().as_ref(), false);
    let main_loop_end = main_loop.clone();

    let join_program = thread::spawn(move || {
        println!("Running program");
        match osx::run(example_main) {
            Ok(_) => println!("Success!"),
            Err(e) => println!("Error! {}", e)
        }
        main_loop_end.quit();
    });

    main_loop.run();
    join_program.join();
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

