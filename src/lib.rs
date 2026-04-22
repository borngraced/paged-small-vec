use std::{mem::MaybeUninit, ptr};

const DEFAULT_CHUNK: usize = 256;
const DEFAULT_INLINE: usize = 10;
const DEFAULT_DIRECT: usize = 20;

pub struct PagedSmallVec<
    T,
    const INLINE: usize = DEFAULT_INLINE,
    const DIRECT: usize = DEFAULT_DIRECT,
    const CHUNK: usize = DEFAULT_CHUNK,
> {
    direct: [Option<Box<[MaybeUninit<T>; CHUNK]>>; DIRECT],
    inline: [MaybeUninit<T>; INLINE],
    len: usize,
    tail_chunk_index: usize,
    tail_offset: usize,
    current_chunk_ptr: *mut MaybeUninit<T>,
}

pub struct ChunkIter<'a, T, const INLINE: usize, const DIRECT: usize, const CHUNK: usize> {
    vec: &'a PagedSmallVec<T, INLINE, DIRECT, CHUNK>,
    remaining: usize,
    next_chunk_index: usize,
    yielded_inline: bool,
}

impl<T, const INLINE: usize, const DIRECT: usize, const CHUNK: usize> Drop
    for PagedSmallVec<T, INLINE, DIRECT, CHUNK>
{
    fn drop(&mut self) {
        for i in 0..self.len {
            if i < INLINE {
                // SAFETY: slots in `0..self.len` are initialized, and this branch only touches
                // the initialized inline prefix.
                unsafe { self.inline.get_unchecked_mut(i).assume_init_drop() };
                continue;
            }

            let paged_index = i - INLINE;
            let chunk_index = paged_index / CHUNK;
            let offset = paged_index % CHUNK;

            // SAFETY: slots in `0..self.len` are initialized, so the corresponding direct chunk
            // exists and `offset` points at an initialized element.
            unsafe {
                self.direct[chunk_index]
                    .as_mut()
                    .expect("chunk must exist for initialized index")[offset]
                    .assume_init_drop();
            }
        }
    }
}

impl<T> PagedSmallVec<T, DEFAULT_INLINE, DEFAULT_DIRECT, DEFAULT_CHUNK> {
    pub const fn new() -> PagedSmallVec<T, DEFAULT_INLINE, DEFAULT_DIRECT, DEFAULT_CHUNK> {
        Self {
            len: 0,
            inline: [const { MaybeUninit::uninit() }; DEFAULT_INLINE],
            direct: [const { None }; DEFAULT_DIRECT],
            tail_chunk_index: 0,
            tail_offset: 0,
            current_chunk_ptr: ptr::null_mut(),
        }
    }
}

impl<T, const INLINE: usize, const DIRECT: usize, const CHUNK: usize>
    PagedSmallVec<T, INLINE, DIRECT, CHUNK>
{
    pub const fn with_layout() -> PagedSmallVec<T, INLINE, DIRECT, CHUNK> {
        Self {
            len: 0,
            inline: [const { MaybeUninit::uninit() }; INLINE],
            direct: [const { None }; DIRECT],
            tail_chunk_index: 0,
            tail_offset: 0,
            current_chunk_ptr: ptr::null_mut(),
        }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn chunks(&self) -> ChunkIter<'_, T, INLINE, DIRECT, CHUNK> {
        ChunkIter {
            vec: self,
            remaining: self.len,
            next_chunk_index: 0,
            yielded_inline: false,
        }
    }

    #[inline(always)]
    pub fn for_each_chunk(&self, mut f: impl FnMut(&[T])) {
        for chunk in self.chunks() {
            f(chunk);
        }
    }

    #[inline(always)]
    pub fn for_each_ref(&self, mut f: impl FnMut(&T)) {
        self.for_each_chunk(|chunk| {
            for value in chunk {
                f(value);
            }
        });
    }

    pub fn push(&mut self, val: T) {
        assert!(self.len < INLINE + DIRECT * CHUNK, "stage 1 occupied");

        if self.len < INLINE {
            self.inline[self.len].write(val);
        } else {
            self.ensure_current_chunk();
            // SAFETY: `ensure_current_chunk` sets `current_chunk_ptr` to the chunk covering the
            // current tail position, and `tail_offset < CHUNK`.
            unsafe { (*self.current_chunk_ptr.add(self.tail_offset)).write(val) };
            self.advance_tail();
        }

        self.len += 1;
    }

    pub fn extend_from_slice(&mut self, values: &[T])
    where
        T: Clone,
    {
        assert!(
            self.len + values.len() <= INLINE + DIRECT * CHUNK,
            "stage 1 occupied"
        );

        let mut values = values.iter();

        while self.len < INLINE {
            let Some(value) = values.next() else {
                return;
            };
            self.inline[self.len].write(value.clone());
            self.len += 1;
        }

        while let Some(value) = values.next() {
            self.ensure_current_chunk();
            // SAFETY: `ensure_current_chunk` sets `current_chunk_ptr` to the chunk covering the
            // current tail position, and `tail_offset < CHUNK`.
            unsafe { (*self.current_chunk_ptr.add(self.tail_offset)).write(value.clone()) };
            self.advance_tail();
            self.len += 1;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len < 1 {
            return None;
        }

        let popped = if self.len <= INLINE {
            let index = self.len - 1;
            // SAFETY: `index < self.len`, so this inline slot is initialized.
            unsafe { self.inline.get_unchecked(index).assume_init_read() }
        } else {
            self.rewind_tail();
            self.refresh_current_chunk_ptr_for_read();
            // SAFETY: after rewinding, the cached tail points at the initialized last paged slot.
            unsafe { (&*self.current_chunk_ptr.add(self.tail_offset)).assume_init_read() }
        };

        self.len -= 1;
        Some(popped)
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        // SAFETY: the bounds check above guarantees `index < self.len()`.
        Some(unsafe { self.get_unchecked(index) })
    }

    /// Returns a reference to the element at `index` without bounds checks.
    ///
    /// # Safety
    /// The caller must ensure that `index < self.len()`.
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        if index < INLINE {
            // SAFETY: the caller guarantees `index < self.len()`,
            // and this branch only touches an initialized inline slot.
            return unsafe { self.inline.get_unchecked(index).assume_init_ref() };
        }

        let paged_index = index - INLINE;
        let chunk_index = paged_index / CHUNK;
        let offset = paged_index % CHUNK;

        // SAFETY: the caller guarantees `index < self.len()`, so the computed
        // direct chunk exists and `offset` points at an initialized element.
        unsafe {
            self.direct
                .get_unchecked(chunk_index)
                .as_ref()
                .unwrap_unchecked()
                .get_unchecked(offset)
                .assume_init_ref()
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        let removed = self.take_slot(index);

        for i in (index + 1)..self.len {
            let val = self.take_slot(i);
            self.write_slot(i - 1, val);
        }

        self.len -= 1;
        self.refresh_tail();
        Some(removed)
    }

    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        let last = self.len - 1;
        let removed = self.take_slot(index);
        if index != last {
            let last_value = self.take_slot(last);
            self.write_slot(index, last_value);
        }

        self.len -= 1;
        self.refresh_tail();
        Some(removed)
    }

    fn take_slot(&mut self, index: usize) -> T {
        if index < INLINE {
            // SAFETY: callers only pass initialized logical indices; this branch reads an
            // initialized inline slot.
            return unsafe { self.inline.get_unchecked(index).assume_init_read() };
        }

        let paged_index = index - INLINE;
        let chunk_index = paged_index / CHUNK;
        let offset = paged_index % CHUNK;

        // SAFETY: callers only pass initialized logical indices, so the computed direct chunk
        // exists and `offset` points at an initialized element.
        unsafe {
            self.direct
                .get_unchecked_mut(chunk_index)
                .as_mut()
                .expect("chunk must exist for initialized index")[offset]
                .assume_init_read()
        }
    }

    fn write_slot(&mut self, index: usize, val: T) {
        if index < INLINE {
            self.inline[index].write(val);
        } else {
            let paged_index = index - INLINE;
            let chunk_index = paged_index / CHUNK;
            let offset = paged_index % CHUNK;

            let chunk = self.direct[chunk_index]
                .get_or_insert_with(|| Box::new([const { MaybeUninit::uninit() }; CHUNK]));
            // SAFETY: `offset` is `paged_index % CHUNK`, so it is within `CHUNK`.
            unsafe { chunk.get_unchecked_mut(offset).write(val) };
        }
    }

    #[inline(always)]
    fn ensure_current_chunk(&mut self) {
        if !self.current_chunk_ptr.is_null() {
            return;
        }

        let chunk = self.direct[self.tail_chunk_index]
            .get_or_insert_with(|| Box::new([const { MaybeUninit::uninit() }; CHUNK]));
        self.current_chunk_ptr = chunk.as_mut_ptr();
    }

    #[inline(always)]
    fn advance_tail(&mut self) {
        self.tail_offset += 1;
        if self.tail_offset == CHUNK {
            self.tail_offset = 0;
            self.tail_chunk_index += 1;
            self.current_chunk_ptr = ptr::null_mut();
        }
    }

    #[inline(always)]
    fn rewind_tail(&mut self) {
        if self.tail_offset == 0 {
            self.tail_chunk_index -= 1;
            self.tail_offset = CHUNK - 1;
            self.current_chunk_ptr = ptr::null_mut();
        } else {
            self.tail_offset -= 1;
        }
    }

    #[inline(always)]
    fn refresh_tail(&mut self) {
        if self.len <= INLINE {
            self.tail_chunk_index = 0;
            self.tail_offset = 0;
            self.current_chunk_ptr = ptr::null_mut();
            return;
        }

        let paged_len = self.len - INLINE;
        self.tail_chunk_index = paged_len / CHUNK;
        self.tail_offset = paged_len % CHUNK;
        self.refresh_current_chunk_ptr_for_write();
    }

    #[inline(always)]
    fn refresh_current_chunk_ptr_for_write(&mut self) {
        if self.len < INLINE || self.len >= INLINE + DIRECT * CHUNK || self.tail_offset == 0 {
            self.current_chunk_ptr = ptr::null_mut();
            return;
        }

        self.current_chunk_ptr = self.direct[self.tail_chunk_index]
            .as_mut()
            .map_or(ptr::null_mut(), |chunk| chunk.as_mut_ptr());
    }

    #[inline(always)]
    fn refresh_current_chunk_ptr_for_read(&mut self) {
        self.current_chunk_ptr = self.direct[self.tail_chunk_index]
            .as_mut()
            .map_or(ptr::null_mut(), |chunk| chunk.as_mut_ptr());
    }
}

impl<'a, T, const INLINE: usize, const DIRECT: usize, const CHUNK: usize> Iterator
    for ChunkIter<'a, T, INLINE, DIRECT, CHUNK>
{
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        if !self.yielded_inline {
            self.yielded_inline = true;
            let inline_len = self.remaining.min(INLINE);
            if inline_len > 0 {
                self.remaining -= inline_len;
                // SAFETY: the first `inline_len` inline slots are initialized and laid out like `T`.
                return Some(unsafe {
                    std::slice::from_raw_parts(self.vec.inline.as_ptr().cast::<T>(), inline_len)
                });
            }
        }

        let chunk_len = self.remaining.min(CHUNK);
        // SAFETY: `self.remaining > 0` means this direct chunk is within the initialized logical
        // range, so the chunk exists.
        let chunk = unsafe {
            self.vec
                .direct
                .get_unchecked(self.next_chunk_index)
                .as_ref()
                .unwrap_unchecked()
        };
        self.remaining -= chunk_len;
        self.next_chunk_index += 1;

        // SAFETY: the first `chunk_len` elements in this chunk are initialized and laid out like `T`.
        Some(unsafe { std::slice::from_raw_parts(chunk.as_ptr().cast::<T>(), chunk_len) })
    }
}

#[test]
fn test_paged_small_vec() {
    let mut vec = PagedSmallVec::new();

    for i in 1..20 {
        vec.push(i)
    }

    assert_eq!(Some(5), vec.remove(4));
    assert_eq!(Some(&6), vec.get(4))
}

#[test]
fn test_for_each_chunk_preserves_order() {
    let mut vec = PagedSmallVec::<u32, 4, 2>::with_layout();
    for i in 0..10 {
        vec.push(i);
    }

    let mut seen = Vec::new();
    vec.for_each_chunk(|chunk| seen.extend_from_slice(chunk));

    assert_eq!(seen, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_chunks_preserve_order() {
    let mut vec = PagedSmallVec::<u32, 4, 2>::with_layout();
    vec.extend_from_slice(&(0..10).collect::<Vec<_>>());

    let seen = vec
        .chunks()
        .flat_map(|chunk| chunk.iter().copied())
        .collect::<Vec<_>>();

    assert_eq!(seen, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_extend_from_slice_appends_in_order() {
    let mut vec = PagedSmallVec::<u32, 4, 2>::with_layout();
    vec.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7]);

    let seen = vec
        .chunks()
        .flat_map(|chunk| chunk.iter().copied())
        .collect::<Vec<_>>();

    assert_eq!(seen, vec![1, 2, 3, 4, 5, 6, 7]);
}
