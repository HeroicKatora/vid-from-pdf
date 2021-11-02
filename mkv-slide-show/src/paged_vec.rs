use std::{cell::RefCell, io::Write, ops::Deref, rc::Rc};
use webm_iterable::WebmWriter;

pub struct PagedVec {
    // FIXME: should have a limited buffer size, not exposed.
    pub(crate) inner: Shared,
    /// Because we must keep the writer alive (it contains internal state for the begin of open
    /// tags that we can't reconstruct) we can not borrow from ourselves. But it also does not
    /// offer any access.
    /// Hence we write into a shared buffer.
    pub(crate) writer: WebmWriter<Shared>,
}

#[derive(Clone)]
pub(crate) struct Shared {
    inner: Rc<RefCell<Vec<u8>>>,
}

impl PagedVec {
    pub fn default_memory() -> usize {
        1_000_000
    }

    pub fn new(mem: usize) -> Self {
        let vec = Vec::with_capacity(mem);
        let inner = Shared::from(vec);
        let writer = WebmWriter::new(inner.clone());
        PagedVec { inner, writer }
    }

    pub fn ready(&self) -> impl Deref<Target=[u8]> + '_ {
        let reference = self.inner.inner.borrow();
        std::cell::Ref::map(reference, |v| &**v)
    }

    pub fn consume(&mut self, len: usize) {
        self.inner.inner
            .borrow_mut()
            .splice(..len, core::iter::empty())
            .for_each(drop);
    }

    pub(crate) fn writer(&mut self)
        -> &'_ mut webm_iterable::WebmWriter<impl Write>
    {
        &mut self.writer
    }
}

impl From<Vec<u8>> for Shared {
    fn from(vec: Vec<u8>) -> Self {
        Shared { inner: Rc::new(RefCell::new(vec)) }
    }
}

impl Write for Shared {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
