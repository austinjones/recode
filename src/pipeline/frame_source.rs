use pipeline::pipeline_utils::*;
use audio::audio_buffer::*;
use video::video_buffer::*;

use audio::audio_format::*;
use video::video_format::*;

use audio::audio_iter::*;
use video::video_iter::*;

use std::i32;
use std::thread;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::sync_channel;


use gstreamer;
use gstreamer_app;
use gstreamer_audio;
use gstreamer_video;
use gstreamer::prelude::*;


#[macro_use]
use gstreamer::Element;

use byte_slice_cast::*;

extern crate failure;
use failure::Error;

pub struct FrameSource {
    pipeline: gstreamer::Pipeline,
    arx: Option<Receiver<Arc<Mutex<AudioBuffer>>>>,
    vrx: Option<Receiver<Arc<Mutex<VideoBuffer>>>>
}

impl IntoPipeline for FrameSource {
    fn into_pipeline(&self) -> &gstreamer::Pipeline {
        &self.pipeline
    }
}

impl FrameSource {
    pub fn raw_audio_caps() -> gstreamer::Caps {
        gstreamer::Caps::new_simple(
            "audio/x-raw",
            &[
                ("format", &gstreamer_audio::AUDIO_FORMAT_S16.to_string()),
                ("layout", &"interleaved"),
                ("channels", &gstreamer::IntRange::<i32>::new(1, i32::MAX)),
                ("rate", &48000),
                ("channel-mask", &gstreamer::Bitmask::new(0x0000000000000003))
            ],
        )
    }

    // pub fn raw_audio_caps_output() -> gstreamer::Caps {
    //     gstreamer::Caps::new_simple(
    //         "audio/x-unaligned-raw",
    //         &[
    //             ("format", &gstreamer_audio::AUDIO_FORMAT_S32.to_string()),
    //             ("layout", &"interleaved"),
    //             ("channels", &gstreamer::IntRange::<i32>::new(1, i32::MAX)),
    //             ("rate", &gstreamer::IntRange::<i32>::new(1, i32::MAX))
    //         ],
    //     )
    // }

    pub fn raw_video_caps() -> gstreamer::Caps {
        gstreamer::Caps::new_simple(
            "video/x-raw", 
            &[
                ("format", &"AYUV"),
                ("interlace-mode", &"progressive"),
                ("pixel-aspect-ratio", &gstreamer::Fraction::new(1,1)), 
                ("chroma-site", &"mpeg2"), 
                ("colorimetry", &"bt709")
            ]
        )
    }

    pub fn new(uri: &str) -> Result<(FrameSource,Receiver<Arc<Mutex<AudioBuffer>>>,Receiver<Arc<Mutex<VideoBuffer>>>), Error> {
        let pipeline = gstreamer::Pipeline::new("recode-input");

        let src = gstreamer::ElementFactory::make("filesrc", None).ok_or(MissingElement("filesrc"))?;
        src.set_property("location", &uri)?;

        let mut frameSource = FrameSource {  
            pipeline: pipeline,
            arx: None,
            vrx: None
        };
        let (arx,vrx) = frameSource.register_appsinks(&src)?;

        return Ok((frameSource, arx, vrx));
    }

    fn register_appsinks(&mut self, src: &gstreamer::Element) -> Result<(Receiver<Arc<Mutex<AudioBuffer>>>,Receiver<Arc<Mutex<VideoBuffer>>>),Error> {
        let decodebin =
            gstreamer::ElementFactory::make("decodebin", None).ok_or(MissingElement("decodebin"))?;

        let audiosink = gstreamer::ElementFactory::make("appsink", None).unwrap();
        let videosink = gstreamer::ElementFactory::make("appsink", None).unwrap();

        let videoconvert = gstreamer::ElementFactory::make("videoconvert", None).unwrap();
        let audioconvert = gstreamer::ElementFactory::make("audioconvert", None).unwrap();
        let audioresample = gstreamer::ElementFactory::make("audioresample", None).unwrap();

        let audiosink_appsink = 
            audiosink
            .dynamic_cast::<gstreamer_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");

        audiosink_appsink.set_caps(&Self::raw_audio_caps());

        let videosink_appsink = 
            videosink
            .dynamic_cast::<gstreamer_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");

        videosink_appsink.set_caps(&Self::raw_video_caps());

        let video_format = Arc::new(Mutex::new(VideoFormat::empty()));
        let audio_format = Arc::new(Mutex::new(AudioFormat::empty()));
        let vf1 = video_format.clone();
        let af1 = audio_format.clone();

        let (vtx, vrx) = sync_channel(8);
        let vtx_mutex = Mutex::new(vtx);
        videosink_appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::new()
                .new_sample(move |appsink| {
                    let sample = match appsink.pull_sample() {
                        None => return gstreamer::FlowReturn::Eos,
                        Some(sample) => sample,
                    };

                    let buffer = if let Some(buffer) = sample.get_buffer() {
                        buffer
                    } else {
                        gst_element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to get buffer from appsink")
                        );

                        return gstreamer::FlowReturn::Error;
                    };

                    let samples = if let Some(map) = buffer.map_readable() {
                        map
                    } else {
                        gst_element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to map buffer readable")
                        );

                        return gstreamer::FlowReturn::Error;
                    };

                    // let caps = sample.get_caps().unwrap();
                    // let caps_structure = caps.get_structure(0).unwrap();
                    // let framerate = caps_structure.get::<gst::Fraction>("framerate").unwrap();
                    // let width = caps_structure.get::<i32>("width").unwrap();
                    // let height = caps_structure.get::<i32>("height").unwrap();
                    
                    // println!("Caps structure : {:?}",caps_structure);
                    // println!("Got video sample with framerate: {:?}", fps);


                    // TODO: extract into new
                    let video_buffer = VideoBuffer {
                        buffer: samples.to_vec().clone(),
                        format: vf1.lock().unwrap().clone(),
                        time: buffer.get_pts().nanoseconds().unwrap_or(0u64) as f64 / 1_000_000_000f64,
                        clock_time: buffer.get_pts(),
                        duration: buffer.get_duration()
                        // framerate: framerate,
                        // width: width,
                        // height: height
                    };

                    // println!("Captured video buffer at time {:?}", video_buffer.time);

                    vtx_mutex.lock().unwrap().send(Arc::new(Mutex::new(video_buffer)));
                    
                    gstreamer::FlowReturn::Ok
                })
                .build()
        );

        let (atx, arx) = sync_channel(8);
        let atx_mutex = Mutex::new(atx);
        audiosink_appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::new()
                .new_sample(move |appsink| {
                    let sample = match appsink.pull_sample() {
                        None => return gstreamer::FlowReturn::Eos,
                        Some(sample) => sample,
                    };

                    let buffer = if let Some(buffer) = sample.get_buffer() {
                        buffer
                    } else {
                        gst_element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to get buffer from appsink")
                        );

                        return gstreamer::FlowReturn::Error;
                    };
                    println!("IN  {}", buffer.get_pts());

                    let vec;
                    {
                        let map = if let Some(map) = buffer.map_readable() {
                            map
                        } else {
                            gst_element_error!(
                                appsink,
                                gstreamer::ResourceError::Failed,
                                ("Failed to map buffer readable")
                            );

                            return gstreamer::FlowReturn::Error;
                        };

                        // println!("Buffer length: {}", map.len());
                        let samples = if let Ok(samples) = map.as_slice().as_slice_of::<i16>() {
                            samples
                        } else {
                            gst_element_error!(
                                appsink,
                                gstreamer::ResourceError::Failed,
                                ("Failed to interprete buffer as i32 PCM")
                            );
                    
                            return gstreamer
                            ::FlowReturn::Error;
                        };

                        vec = samples.to_vec().clone();
                    }
                    

                    // let caps = sample.get_caps().unwrap();
                    // let caps_structure = caps.get_structure(0).unwrap();
                    // let rate = caps_structure.get::<i32>("rate").unwrap();
                    // let channels = caps_structure.get::<i32>("channels").unwrap();

                    // let segment = sample.get_segment().unwrap();
                    // let start = segment.get_start();
                    
                    // println!("Got audio sample with rate {:?}, channels {:?}", rate, channels);
                    
                    let format = af1.lock().unwrap().clone();
                    let buffer = AudioBuffer::new(buffer, vec, format);

                    // println!("Captured audio buffer at time {:?}", buffer.time);

                    atx_mutex.lock().unwrap().send(Arc::new(Mutex::new(buffer)));
                    
                    gstreamer::FlowReturn::Ok
                })
                .build()
        );

        self.pipeline.add_many(&[src, &decodebin])?;
        self.pipeline.add_many(&[&audioconvert, &audioresample, &videoconvert])?;
        self.pipeline.add_many(&[&audiosink_appsink, &videosink_appsink])?;
        gstreamer::Element::link_many(&[src, &decodebin])?;
        
        // hacky concurrency here.
        // I am betting that the connect pad will be available before the appsink callbacks are triggered

        // Need to move a new reference into the closure
        decodebin.connect_pad_added(move |element, src_pad| {
            let caps = src_pad.get_current_caps();
            if caps.is_none() {
                return;
            }

            let caps = caps.unwrap();
            for structure in caps.iter() {
                let name = structure.get_name();
                println!("{:?}", structure);
                if name.starts_with("audio/") {
                    println!("Audio structure: {:?}", structure);
                    let rate = structure.get::<i32>("rate").unwrap();
                    let channels = structure.get::<i32>("channels").unwrap();

                    let mut audstr = audio_format.lock().unwrap();
                    *audstr = AudioFormat::new(rate, channels);

                    match element.link(&audioconvert) {
                        Ok(_) => println!("Connected audio pad: {}", name),
                        Err(e) => println!("Error connecting audio pad: {}", e)
                    }

                    match audioconvert.link(&audioresample) {
                        Ok(_) => println!("Connected audio pad: {}", name),
                        Err(e) => println!("Error connecting audio pad: {}", e)
                    }

                    match audioresample.link(&audiosink_appsink) {
                        Ok(_) => println!("Connected audio conversion to audio sink"),
                        Err(e) => println!("Error connecting audio conversion: {}", e)
                    }

                    audioconvert.sync_state_with_parent();
                    audioresample.sync_state_with_parent();
                }
                
                if name.starts_with("video/") {
                    let framerate = structure.get::<gstreamer::Fraction>("framerate").unwrap();
                    let width = structure.get::<i32>("width").unwrap();
                    let height = structure.get::<i32>("height").unwrap();

                    let mut videostr = video_format.lock().unwrap();
                    *videostr = VideoFormat::new(framerate, width, height);

                    match element.link(&videoconvert) {
                        Ok(_) =>  println!("Connected video pad: {}", name),
                        Err(e) => println!("Error connecting video pad: {}", e)
                    }

                    match videoconvert.link(&videosink_appsink) {
                        Ok(_) => println!("Connected video conversion to video sink"),
                        Err(e) => println!("Error connecting video conversion: {}", e)
                    }
                    
                    videoconvert.sync_state_with_parent();
                }
            }
        });

        return Ok((arx, vrx));
    }

    fn handle_video_frame(&self, timecode: i32, frame: Vec<u8>) {

    }

    fn handle_audio_sample(&self, timecode: i32, sample: i32, channel: u16) {

    }
}