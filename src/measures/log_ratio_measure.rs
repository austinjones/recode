use measures::mean_window_measure::*;
use measures::*;
use stats::mean;

pub struct LogRatioMeasure {

}

impl<'a> LogRatioMeasure {
    pub fn new() -> LogRatioMeasure {
        LogRatioMeasure {
            // edge: MeanWindowMeasure::new(signal.clone(), signal_default, signal_window),
            // baseline: MeanWindowMeasure::new(signal, signal_default, baseline_window)
        }
    }
}

// impl<T> WrappingMeasure<T> for AudioEdgeMeasure<T> {
//     fn inner(&self) -> &T {
//         &self.baseline
//     }

//     fn inner_mut(&mut self) -> &mut T {
//         &mut self.baseline
//     }
// }

impl<'a> Measure<(f64, f64), f64> for LogRatioMeasure {
    fn value(&mut self, (edge, avg): (f64, f64)) -> f64 {    
        if avg == 0f64 {
            return ::std::f64::NAN;   
        }
        
        // we take the natural log to help with smoothing/normalization
        // ratios from [0, inf) don't normalize well in sigmoids
        // ln remaps this to (-inf, inf), which has symmetry around point 0
        (edge / avg).ln()
    }
}