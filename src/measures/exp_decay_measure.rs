use measures::*;

// mod tests {
//     use measures::test_measure::TestMeasure;
//     use measures::exp_decay_measure::*;

//     fn test_measure() -> TestMeasure {
//         TestMeasure {
//             vals: vec![0f64, 2f64, 4f64, 0f64],
//             times: vec![0f64, 1f64, 2f64, 3f64],
//             index: 0usize
//         }
//     }

//     fn test_measure_1sec() -> TestMeasure {
//         TestMeasure {
//             vals: vec![0f64, 2f64, 2f64, 2f64, 2f64, 2f64, 2f64],
//             times: vec![0f64, 0.2f64, 0.4f64, 0.6f64, 0.8f64, 1f64],
//             index: 0usize
//         }
//     }

//     #[test]
//     fn simple() {
//         let test_measure = test_measure();
//         let mut exp = ExpDecayMeasure::new(test_measure, 1f64, 0.25f64);

//         exp.update(&());
//         let val = exp.value();
//         exp.update(&());
//         let val1 = exp.value();
//         exp.update(&());
//         let val2 = exp.value();
//         exp.update(&());
//         let val3 = exp.value();
//         println!("Val (0s): {}", val);
//         println!("Val2 (1s): {}", val1);
//         println!("Val3 (2s): {}", val2);
//         println!("Val4 (3s): {}", val3);

//         let test_measure = test_measure_1sec();
//         let mut exp = ExpDecayMeasure::new(test_measure, 1f64, 0.25f64);

//         exp.update(&());
//         let val = exp.value();
//         exp.update(&());
//         let val02 = exp.value();
//         exp.update(&());
//         let val04 = exp.value();
//         exp.update(&());
//         let val06 = exp.value();
//         exp.update(&());
//         let val08 = exp.value();
//         exp.update(&());
//         let val10 = exp.value();
//         println!("Val (0s): {}", val);
//         println!("Val02 (0.2s): {}", val02);
//         println!("Val04 (0.4s): {}", val04);
//         println!("Val06 (0.6s): {}", val06);
//         println!("Val08 (0.8s): {}", val08);
//         println!("Val10 (1.0s): {}", val10);
//     }

//     #[test]
//     fn test_4sec() {

//     }
// }

pub struct ExpDecayMeasure {
    up_timeconst: f64,
    down_timeconst: f64,
    last_time: f64,
    last_value: f64
}

impl ExpDecayMeasure {
    pub fn new(up_timeconst: f64, down_timeconst: f64) -> ExpDecayMeasure {
        ExpDecayMeasure {
            up_timeconst: up_timeconst,
            down_timeconst: down_timeconst,
            last_time: ::std::f64::NAN,
            last_value: ::std::f64::NAN
        }
    }
}

// impl<T> WrappingMeasure<T> for ExpDecayMeasure<T> {
//     fn inner(&self) -> &T {
//         &self.inner
//     }

//     fn inner_mut(&mut self) -> &mut T {
//         &mut self.inner
//     }
// }

impl ExpDecayMeasure {
    
}

impl StatefulMeasure<(f64, f64), f64> for ExpDecayMeasure {
    fn value(&mut self) -> f64 {
        self.last_value
    }

    fn update(&mut self, (new_value, new_time): (f64, f64)) {
        if self.last_time.is_nan() || self.last_value.is_nan() {
            self.last_time = new_time;
            self.last_value = new_value;
            return;
        }


        // let value_diff = (new_value - self.last_value).abs();
        let time_diff = new_time - self.last_time;

        if time_diff == 0f64 {
            return;
        }

        let time_const = if new_value > self.last_value { self.up_timeconst } else { self.down_timeconst };
        let alpha = 1f64 - ::std::f64::consts::E.powf(-1f64 * time_diff / time_const); 
        let new_val = alpha * new_value + (1f64 - alpha) * self.last_value;
        // this isn't perfect, but we'll see if it introduces artifacts.
        // let sign = if new_value > self.last_value { 1f64 } else { -1f64 };
        // let scale = if new_value > self.last_value { self.up_scale } else { self.down_scale };
        // let exp_val = self.last_value + sign * adjust * value_diff;
        // println!("Sign: {}, Scale: {}, time_diff: {}, adjust: {}, exp_val: {}", sign, scale, time_diff, adjust, exp_val);
        // println!("New val: {}, alpha: {}", new_val, alpha);

        self.last_time = new_time;
        self.last_value = new_val;
    }
}