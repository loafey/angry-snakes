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
        self.inner[self.index] = rand::random();
        self.index = (self.index + 1) % N;
        val
    }
}
