pub struct TickBuffer<const N: usize> {
    inner: [u64; N],
    index: usize,
}
impl<const N: usize> std::fmt::Debug for TickBuffer<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
impl<const N: usize> TickBuffer<N> {
    pub fn new() -> Self {
        let mut inner = [0; N];
        (0..N).for_each(|i| inner[i] = rand::random());
        Self { inner, index: 0 }
    }
    pub fn next(&mut self) -> u64 {
        let val = self.inner[self.index];
        self.index = (self.index + 1) % N;
        self.inner[self.index] = rand::random();
        val
    }
    pub fn current(&self) -> u64 {
        self.inner[self.index]
    }
    pub fn since(&self, val: u64) -> Option<usize> {
        let mut check = self.index.wrapping_sub(1) % self.inner.len();
        let mut steps = 0;
        while self.index != check {
            steps += 1;
            if self.inner[check] == val {
                return Some(steps);
            }
            check = check.wrapping_sub(1) % self.inner.len();
        }
        None
    }
}

mod tests {
    #[test]
    fn overflow() {
        const S: usize = 50;
        let mut tb: super::TickBuffer<S> = super::TickBuffer::new();
        for _ in 0..(S * 5000) {
            let curr = tb.current();
            tb.next();
            let mut i = 1;
            while let Some(x) = tb.since(curr) {
                assert_eq!(x, i);
                tb.next();
                i += 1;
            }
        }
    }
}
