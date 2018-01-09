pub struct QueueBuf<T> {
    pos: usize,
    length: usize,
    pub vec: Vec<T>
}

impl<T: Copy> QueueBuf<T> {
    pub fn new(buf: Vec<T>) -> QueueBuf<T> {
        QueueBuf {
            pos: 0,
            length: 0,
            vec: buf
        }
    }

    fn get(&self, index: usize) -> Option<&T> {
        let idx = self.index_to_vec(index);
        idx.map(|e| self.vec.get(e).unwrap())
    }

    fn index_to_vec(&self, index: usize)  -> Option<usize> {
        if index + 1 > self.length {
            return None;
        }

        if index < self.pos {
            Some(self.pos - index - 1)
        } else {
            Some(self.vec.len() - 1 - index + self.pos)
        }
    }

    pub fn is_saturated(&self) -> bool {
        self.length >= self.vec.len()
    }

    pub fn push(&mut self, val: T) {
        if self.pos + 1 > self.vec.len() {
            self.pos = 0;
        }

        self.vec[self.pos] = val;

        self.pos += 1;
        self.length += 1;
    }

    pub fn extract(&self) -> Vec<T> {
        if self.vec.is_empty() {
            return Vec::with_capacity(0);
        }

        let mut vec = Vec::with_capacity(self.vec.len());
        let slice1 = &self.vec.as_slice()[self.pos..];
        let slice2 = &self.vec.as_slice()[..self.pos];

        vec.extend(slice1.iter().cloned());
        vec.extend(slice2.iter().cloned());
        vec
    }
}

impl QueueBuf<f64> {
    pub fn mean(&self, last_n: usize, default: f64) -> f64 {
        if last_n > self.vec.len() {
            panic!("Insufficent capacity in queue_buf for mean")
        }
        
        let mut sum = 0f64;
        let mut n = 0;

        for i in 0..last_n {
            sum += self.get(i).unwrap_or(&default);
            n += 1;
        }

        sum / (n as f64)
    }
}