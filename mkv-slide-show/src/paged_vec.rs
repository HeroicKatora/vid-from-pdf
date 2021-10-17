pub struct PagedVec {
    // FIXME: should have a limited buffer size, not exposed.
    pub(crate) inner: Vec<u8>,
}

impl PagedVec {
    pub fn default_memory() -> usize {
        1_000_000
    }

    pub fn new(mem: usize) -> Self {
        PagedVec { inner: Vec::with_capacity(mem) }
    }

    pub fn ready(&self) -> &[u8] {
        &self.inner
    }

    pub fn consume(&mut self, len: usize) {
        self.inner.splice(..len, core::iter::empty())
            .for_each(drop);
    }

    pub(crate) fn writer(&mut self)
        -> webm_iterable::WebmWriter<&'_ mut dyn std::io::Write>
    {
        webm_iterable::WebmWriter::new(&mut self.inner)
    }
}
