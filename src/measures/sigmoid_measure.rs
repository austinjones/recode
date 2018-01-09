use measures::*;
use stats::OnlineStats;

pub enum SigmoidRange {
    ZeroToOne,
    NegOneToOne
}

pub struct SigmoidMeasure {
    range: SigmoidRange,
    stats: OnlineStats
}

impl SigmoidMeasure {
    pub fn new(range: SigmoidRange) -> SigmoidMeasure {
        SigmoidMeasure {
            range: range,
            stats: OnlineStats::new()
        }
    }

    pub fn sigmoid(x: f64) -> f64 {
        1f64 / (1f64 + ::std::f64::consts::E.powf(-1f64 * x))
    }
}

impl Measure<f64, f64> for SigmoidMeasure {
    fn value(&mut self, val: f64) -> f64 {
        if !val.is_nan() {
            self.stats.add(val);
        }
        
        let mut stddev = self.stats.stddev();
        if stddev == 0f64 {
            stddev = 1f64;
        }

        let adjust_scale = val / stddev;
        let sig = Self::sigmoid(adjust_scale);
        match &self.range {
            &SigmoidRange::ZeroToOne => sig,
            &SigmoidRange::NegOneToOne => 2f64 * (sig-0.5f64)
        }
    }
}

// impl<T> WrappingMeasure<T> for SigmoidMeasure<T> {
//     fn inner(&self) -> &T {
//         &self.inner
//     }

//     fn inner_mut(&mut self) -> &mut T {
//         &mut self.inner
//     }
// }