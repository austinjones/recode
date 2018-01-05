extern crate gstreamer as gst;
use gstreamer::prelude::*;

extern crate glib;

extern crate failure;
use failure::Error;
use std::error::Error as StdError;

#[derive(Debug, Fail)]
#[fail(display = "Missing element {}", _0)]
pub struct MissingElement(pub &'static str);

pub struct PipelineUtils {

}

impl PipelineUtils {
    pub fn start<T: IntoPipeline>(into: &T) -> Result<(), Error> {
        let pipeline = into.into_pipeline();
        pipeline.set_state(gst::State::Playing).into_result()?;
        Ok(())
    }

    pub fn message<T: IntoPipeline>(into: &T) -> Result<(), Error> {
        let pipeline = into.into_pipeline();        
        let bus = pipeline
            .get_bus()
            .expect("Pipeline without bus. Shouldn't happen!");


        while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
            use gstreamer::MessageView;

            match msg.view() {
                MessageView::Eos(_) => break,
                MessageView::Error(err) => {
                    pipeline.set_state(gst::State::Null).into_result()?;

                    #[cfg(feature = "v1_10")]
                    {
                        match err.get_details() {
                            Some(details) if details.get_name() == "error-details" => details
                                .get::<&glib::AnySendValue>("error")
                                .cloned()
                                .and_then(|v| {
                                    v.downcast_ref::<Arc<Mutex<Option<Error>>>>()
                                        .and_then(|v| v.lock().unwrap().take())
                                })
                                .map(Result::Err)
                                .expect("error-details message without actual error"),
                            _ => Err(ErrorMessage {
                                src: msg.get_src()
                                    .map(|s| s.get_path_string())
                                    .unwrap_or_else(|| String::from("None")),
                                error: err.get_error().description().into(),
                                debug: err.get_debug(),
                                cause: err.get_error(),
                            }.into()),
                        }?;
                    }
                    #[cfg(not(feature = "v1_10"))]
                    {
                        Err(ErrorMessage {
                            src: msg.get_src()
                                .map(|s| s.get_path_string())
                                .unwrap_or_else(|| String::from("None")),
                            error: err.get_error().description().into(),
                            debug: err.get_debug(),
                            cause: err.get_error(),
                        })?;
                    }
                    break;
                }
                MessageView::StateChanged(s) => {
                    println!(
                        "State changed from {:?}: {:?} -> {:?} ({:?})",
                        msg.get_src().map(|s| s.get_path_string()),
                        s.get_old(),
                        s.get_current(),
                        s.get_pending()
                    );
                }
                _ => (),
            }
        }

        Ok(())
    }

    pub fn stop <T: IntoPipeline>(into: &T) -> Result<(), Error> {
        let pipeline = into.into_pipeline();    
        pipeline.set_state(gst::State::Null).into_result()?;
        Ok(())
    }

    pub fn run<T: IntoPipeline>(into: &T) -> Result<(), Error> {
        println!("Running pipeline!");
        let pipeline = into.into_pipeline();
        Self::start(pipeline);
        Self::message(pipeline);
        Self::stop(pipeline);
        Ok(())
    }
}

pub trait IntoPipeline {
    fn into_pipeline(&self) -> &gst::Pipeline;
}

impl IntoPipeline for gst::Pipeline {
    fn into_pipeline(&self) -> &gst::Pipeline {
        self
    }
}

impl<'a> IntoPipeline for &'a gst::Pipeline {
    fn into_pipeline(&self) -> &gst::Pipeline {
        self
    }
}

#[derive(Debug, Fail)]
#[fail(display = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    pub src: String,
    pub error: String,
    pub debug: Option<String>,
    #[cause] pub cause: glib::Error,
}