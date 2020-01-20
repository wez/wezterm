/// A simple read buffer
pub struct ReadBuffer {
    data: Vec<u8>,
    /// The position to read data from
    pos: usize,
}

impl ReadBuffer {
    pub fn new() -> Self {
        let data = Vec::with_capacity(8192);
        Self { data, pos: 0 }
    }

    pub fn append(&mut self, buf: &[u8]) {
        if self.data.len() + buf.len() > self.data.capacity() {
            if self.pos == self.data.len() {
                self.pos = 0;
            } else if self.pos > 0 {
                let (front, back) = self.data.split_at_mut(self.pos);
                let len = back.len();
                front[0..len].copy_from_slice(back);

                self.pos = len;
                self.data.resize(len, 0);
            }
        }
        self.data.extend_from_slice(buf);
    }

    pub fn avail(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn consume(&mut self, buf: &mut [u8]) -> usize {
        let len = buf.len().min(self.avail());
        if len == 0 {
            0
        } else {
            buf[0..len].copy_from_slice(&self.data[self.pos..self.pos + len]);
            self.pos += len;
            len
        }
    }
}
