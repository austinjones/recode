use measures::*;

pub struct MeanMeasure {
    sum: f64,
    n: usize
}

impl MeanMeasure {
    pub fn new() -> MeanMeasure {
        MeanMeasure {
            sum: 0f64,
            n: 0usize
        }
    }
}

impl StatefulMeasure<f64,f64> for MeanMeasure {
    fn value(&mut self) -> f64 {
        if self.n == 0 {
            return ::std::f64::NAN;
        }

        self.sum / (self.n as f64)
    }

    fn update(&mut self, val: f64) {
        self.sum += val;
        self.n += 1;
    }
}