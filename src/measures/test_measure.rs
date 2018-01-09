use measures::*;

pub struct TestMeasure {
    pub vals: Vec<f64>,
    pub index: usize
}

impl TestMeasure {
    pub fn next(&mut self) {
        self.index += 1;
    }
}

impl Measure<(), f64> for TestMeasure {
    fn value(&mut self, _:()) -> f64 {
        self.vals[self.index - 1]
    }
}