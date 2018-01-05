pub struct QueueBuf<T> {
    pos: usize,
    length: usize,
    vec: Vec<T>
}

impl<T: Copy> QueueBuf<T> {
    pub fn new(buf: Vec<T>) -> QueueBuf<T> {
        QueueBuf {
            pos: 0,
            length: 0,
            vec: buf
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