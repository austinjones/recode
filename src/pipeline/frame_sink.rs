use pipeline::frame_source::*;
use pipeline::frame_transform::*;
use pipeline::pipeline_utils::*;

use audio::audio_buffer::*;
use video::video_buffer::*;

use audio::audio_format::*;
use video::video_format::*;

use audio::audio_iter::*;
use video::video_iter::*;

use std::f64;
use std::thread;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use gstreamer::prelude::*;
use gstreamer;
use gstreamer_app;

extern crate failure;
use failure::Error;

pub struct FrameSink {
    pipeline: gstreamer::Pipeline,
    sink: gstreamer::Element
}

impl IntoPipeline for FrameSink {
    fn into_pipeline(&self) -> &gstreamer::Pipeline {
        &self.pipeline
    }
}

impl FrameSink {
    pub fn spawn<T: FrameTransform + Send + 'static>(uri: &str, transform: 
    T, arx: Receiver<Arc<Mutex<AudioBuffer>>>, vrx: Receiver<Arc<Mutex<VideoBuffer>>>) -> ::std::thread::JoinHandle<()> {
        // get an owned string so the &str doesn't need to exist for the static lifetime...
        // we unpack it on the other side
        let movestr = uri.to_string();
        let mut transform = transform;

        thread::spawn(move || {
            let mut sink = FrameSink::new(movestr.as_str()).unwrap();
            println!("Got framesink!  Waiting for Audio/Video format");

            let mut audio_sink = None;
            let mut video_sink = None;
            //TODO: refactor as closure with generic bounds on AudioIter/VideoIter
            let mut audio_iter = AudioIter::new(arx);
            let mut video_iter = VideoIter::new(vrx);
            
            let audio_format = audio_iter.format();
            println!("Got audio format {:?}!", audio_format);
            audio_sink = Some(sink.add_audio_sink(&audio_format.unwrap()).unwrap());

            let video_format = video_iter.format();
            println!("Got video format {:?}!", video_format);
            video_sink = Some(sink.add_video_sink(&video_format.unwrap()).unwrap());
            // let audio_iter = FrameIterator::new(arx);
            // let video_iter = FrameIterator::new(vrx);

            // let mut audio_iter = arx.into_iter().map(|e| Mutex::into_inner(Arc::try_unwrap(e).ok().unwrap()).ok().unwrap());
            // let mut video_iter = vrx.into_iter().map(|e| Mutex::into_inner(Arc::try_unwrap(e).ok().unwrap()).ok().unwrap());
            // // let audio = arx.into_iter().next();

            // let mut audio = audio_iter.next();
            // let mut video = video_iter.next();

            // let mut audio_frame_iter = audio.iter_mut().flat_map(|e| e.iter_mut());
            // let mut video_frame_iter = video.iter_mut().flat_map(|e| e.iter_mut());
            // // let video = vrx.into_iter()
            // //     .map(|e| *e).flat_map(|e| e.iter());

            // let mut time = 0f64;
            let mut atime = 0f64;
            let mut vtime = 0f64;

            let mut state = 0f64;
            let mut has_audio_frame = true;
            let mut has_video_frame = true;

            PipelineUtils::start(&sink);
            let join = thread::spawn(move || {
                println!("Ran pipeline: {:?}", PipelineUtils::message(&sink));
                println!("Stopping output pipeline");
                PipelineUtils::stop(&sink);
            });
            println!("Write pipeline started!");

            while has_video_frame || has_audio_frame {
                // oh the CPU branch prediction.. poor CPU..
                // however, since this program is meant to do offline 'one time' processing, 
                // the performance is acceptable so far
                // in dev, I just use short sample videos
                if atime < vtime {
                    // println!("Processing audio frame at time {}", atime);
                    match audio_iter.next_audio_frame() {
                        Some(mut frame) => {
                            transform.process_audio_frame(&mut frame, atime);
                            atime = frame.time;
                        },
                        None => {
                            println!("Out of audio frames");
                            has_audio_frame = false;
                            atime = f64::MAX;
                        }
                    }
                } else {
                    // println!("Processing video frame at time {}", atime);
                    match video_iter.next_video_frame() {
                        Some(mut frame) => {
                            transform.process_video_frame(&mut frame, vtime);
                            // let rotation = (256f64 * (vtime % 3f64) / 3f64) as u8;

                            vtime = frame.time;
                            // frame.data[0] = 8;
                        },
                        None => {
                            println!("Out of video frames");
                            has_video_frame = false;
                            vtime = f64::MAX;
                        }
                    }
                }

                while let Some(buf) = video_iter.next_finished_buffer() {
                    println!("Finishing video buffer at time {}", vtime);
                    // println!("Moving video buffer into appsrc...");
                    buf.into_appsrc(video_sink.as_mut().unwrap());
                    // println!("Done moving video buffer into appsrc...");
                }

                while let Some(buf) = audio_iter.next_finished_buffer() {
                    println!("Finishing audio buffer at time {}", atime);
                    buf.into_appsrc(audio_sink.as_mut().unwrap());
                }
                
                // let audio_frame = audio_iter.next();
                // if audio_frame.is_none() {
                //     audio = audio_iter.next();
                    
                // }
                // let video_frame = video_iter.next();
                // println!("Got video width: {:?}", video.format.width);
                // identify next video frame
                // vtime += vf.frame_time;
                
                // process audio to that frame

                // process video frame
            }
            println!("Finished writing frames");
            video_sink.unwrap().end_of_stream();
            audio_sink.unwrap().end_of_stream();

            println!("Waiting for pipeline to stop...");
            join.join();
            println!("Finished write loop");
        })
    }

    pub fn new(uri: &str) -> Result<FrameSink, Error> {
        let pipeline = gstreamer::Pipeline::new("recode-output");
        let filesink = gstreamer::ElementFactory::make("filesink", None).ok_or(MissingElement("filesink"))?;
        filesink.set_property("location", &uri)?;

        pipeline.add_many(&[&filesink])?;

        let encoder = gstreamer::ElementFactory::make("mp4mux", None).ok_or(MissingElement("webmmux"))?;
        encoder.set_property("streamable", &true)?;
        encoder.connect_pad_added(move |element, src_pad| {
            element.link(&filesink);
        });

        pipeline.add_many(&[&encoder])?;

        let sink = FrameSink {
            pipeline: pipeline,
            sink: encoder
        };

        Ok(sink)
    }

    fn add_video_sink(&mut self, format: &VideoFormat) -> Result<gstreamer_app::AppSrc, Error> {
        let src = gstreamer::ElementFactory::make("appsrc", None).ok_or(MissingElement("appsrc"))?;
        src.set_property("block", &true)?;
        let videoconvert =
            gstreamer::ElementFactory::make("videoconvert", None).ok_or(MissingElement("videoconvert"))?;
        let vp9enc = gstreamer::ElementFactory::make("x264enc", None).ok_or(MissingElement("vp9enc"))?;
        vp9enc.set_property("bitrate", &5000u32)?;
        // vp9enc.set_property("threads", &4)?;
        // vp9enc.set_property("target-bitrate", &2_000_000)?;
        let queue = gstreamer::ElementFactory::make("queue", None).ok_or(MissingElement("queue"))?;

        let appsrc = src.clone()
            .dynamic_cast::<gstreamer_app::AppSrc>()
            .expect("Source element is expected to be an appsrc!");

        let mut caps = FrameSource::raw_video_caps();
        {
            let mut_structure = caps.get_mut().unwrap().get_mut_structure(0).unwrap();
            mut_structure.set_value("framerate", format.frame_rate_gst_fraction.to_send_value());
            mut_structure.set_value("width", format.width.to_send_value());
            mut_structure.set_value("height", format.height.to_send_value());
        }

        appsrc.set_caps(&caps);
        appsrc.set_property_format(gstreamer::Format::Time);
        appsrc.set_max_bytes(1024*1024);
        appsrc.set_property_block(true);

        self.pipeline.add_many(&[&src, &queue, &videoconvert, &vp9enc])?;
        // self.pipeline.add_many(&[&src])?;
        src.link(&videoconvert)?;

        let convert_i420_caps = gstreamer::Caps::new_simple(
            "video/x-raw", 
            &[
                ("format", &"I420")
            ]
        );

        videoconvert.link_filtered(&vp9enc, Some(&convert_i420_caps))?;
        vp9enc.link(&queue)?;
        queue.link_pads("src", &self.sink, "video_0")?;

        Ok(appsrc)
    }
    
    pub fn add_audio_sink(&mut self, audio_format: &AudioFormat) -> Result<gstreamer_app::AppSrc, Error> {
        let src = gstreamer::ElementFactory::make("appsrc", None).ok_or(MissingElement("appsrc"))?;
        let audioconvert = gstreamer::ElementFactory::make("audioconvert", None).ok_or(MissingElement("audioconvert"))?;
        let vorbisenc = gstreamer::ElementFactory::make("faac", None).ok_or(MissingElement("vorbisenc"))?;
        vorbisenc.set_property("bitrate", &256_000i32)?;
        let queue = gstreamer::ElementFactory::make("queue", None).ok_or(MissingElement("queue"))?;

        let appsrc = src.clone()
            .dynamic_cast::<gstreamer_app::AppSrc>()
            .expect("Source element is expected to be an appsrc!");

        // let info = gstreamer_audio::AudioInfo::new(gstreamer_audio::AUDIO_FORMAT_i32, format.width as u32, format.height as u32)
        //     .fps(format.frame_rate_gst_fraction)
        //     .build()
        //     .expect("Failed to create video info");

        let mut caps = FrameSource::raw_audio_caps();
        {
            let mut_structure = caps.get_mut().unwrap().get_mut_structure(0).unwrap();
            mut_structure.set_value("channels", audio_format.channels.to_send_value());
            mut_structure.set_value("rate", audio_format.rate.to_send_value());
        }
        appsrc.set_caps(&caps);
        appsrc.set_property_format(gstreamer::Format::Time);
        appsrc.set_max_bytes(1024*1024);
        appsrc.set_property_block(true);

        // self.pipeline.add_many(&[&src, &audioconvert])?;
        self.pipeline.add_many(&[&src, &audioconvert, &vorbisenc, &queue])?;
        src.link(&audioconvert)?;
        audioconvert.link(&vorbisenc)?;
        vorbisenc.link(&queue)?;
        queue.link_pads("src", &self.sink, "audio_0")?;

        Ok(appsrc)
    }
}