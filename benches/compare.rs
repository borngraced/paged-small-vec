use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use paged_small_vec::PagedSmallVec;
use smallvec::SmallVec;

const SMALLVEC_INLINE: usize = 32;
const PAGED_INLINE: usize = 32;
const PAGED_DIRECT: usize = 128;
const PAGED_CHUNK: usize = 256;
const LARGE_N: usize = 2_048;
const POP_EVERY: usize = 8;
const RANDOM_READS: usize = 4_096;
const REMOVE_N: usize = 512;
const PUSH_MICRO_N: usize = 4_096;

trait BenchValue: Clone + 'static {
    fn make(i: usize) -> Self;
    fn score(&self) -> u64;
    fn label() -> &'static str;
}

impl BenchValue for u32 {
    fn make(i: usize) -> Self {
        i as u32
    }

    fn score(&self) -> u64 {
        *self as u64
    }

    fn label() -> &'static str {
        "u32"
    }
}

impl BenchValue for [u8; 64] {
    fn make(i: usize) -> Self {
        [i as u8; 64]
    }

    fn score(&self) -> u64 {
        self[0] as u64
    }

    fn label() -> &'static str {
        "[u8;64]"
    }
}

impl BenchValue for String {
    fn make(i: usize) -> Self {
        format!("value-{i:04}")
    }

    fn score(&self) -> u64 {
        self.len() as u64
    }

    fn label() -> &'static str {
        "String"
    }
}

trait BenchCollection<T>: Sized {
    fn new_empty() -> Self;
    fn build(values: &[T]) -> Self
    where
        T: Clone,
    {
        let mut out = Self::new_empty();
        for value in values {
            out.push(value.clone());
        }
        out
    }

    fn push(&mut self, value: T);
    fn extend_from_slice(&mut self, values: &[T])
    where
        T: Clone,
    {
        for value in values {
            self.push(value.clone());
        }
    }
    fn pop(&mut self) -> Option<T>;
    fn get(&self, index: usize) -> Option<&T>;
    fn remove(&mut self, index: usize) -> Option<T>;
    fn swap_remove(&mut self, index: usize) -> Option<T>;
    fn len(&self) -> usize;
    fn for_each_ref(&self, f: impl FnMut(&T));
    fn name() -> &'static str;
}

impl<T> BenchCollection<T> for Vec<T> {
    fn new_empty() -> Self {
        Vec::new()
    }

    fn push(&mut self, value: T) {
        Vec::push(self, value);
    }

    fn pop(&mut self) -> Option<T> {
        Vec::pop(self)
    }

    fn extend_from_slice(&mut self, values: &[T])
    where
        T: Clone,
    {
        self.extend(values.iter().cloned());
    }

    fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(Vec::remove(self, index))
        } else {
            None
        }
    }

    fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(Vec::swap_remove(self, index))
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn for_each_ref(&self, mut f: impl FnMut(&T)) {
        for value in self {
            f(value);
        }
    }

    fn name() -> &'static str {
        "Vec"
    }
}

impl<T> BenchCollection<T> for SmallVec<[T; SMALLVEC_INLINE]> {
    fn new_empty() -> Self {
        SmallVec::new()
    }

    fn push(&mut self, value: T) {
        SmallVec::push(self, value);
    }

    fn pop(&mut self) -> Option<T> {
        SmallVec::pop(self)
    }

    fn extend_from_slice(&mut self, values: &[T])
    where
        T: Clone,
    {
        self.extend(values.iter().cloned());
    }

    fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(SmallVec::remove(self, index))
        } else {
            None
        }
    }

    fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(SmallVec::swap_remove(self, index))
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        SmallVec::len(self)
    }

    fn for_each_ref(&self, mut f: impl FnMut(&T)) {
        for value in self {
            f(value);
        }
    }

    fn name() -> &'static str {
        "SmallVec"
    }
}

impl<T> BenchCollection<T> for PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, PAGED_CHUNK> {
    fn new_empty() -> Self {
        PagedSmallVec::with_layout()
    }

    fn push(&mut self, value: T) {
        PagedSmallVec::push(self, value);
    }

    fn pop(&mut self) -> Option<T> {
        PagedSmallVec::pop(self)
    }

    fn extend_from_slice(&mut self, values: &[T])
    where
        T: Clone,
    {
        PagedSmallVec::extend_from_slice(self, values);
    }

    fn get(&self, index: usize) -> Option<&T> {
        PagedSmallVec::get(self, index)
    }

    fn remove(&mut self, index: usize) -> Option<T> {
        PagedSmallVec::remove(self, index)
    }

    fn swap_remove(&mut self, index: usize) -> Option<T> {
        PagedSmallVec::swap_remove(self, index)
    }

    fn len(&self) -> usize {
        PagedSmallVec::len(self)
    }

    fn for_each_ref(&self, f: impl FnMut(&T)) {
        PagedSmallVec::for_each_ref(self, f)
    }

    fn name() -> &'static str {
        "PagedSmallVec"
    }
}

struct Paged128<T>(PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, 128>);
struct Paged256<T>(PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, 256>);
struct Paged512<T>(PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, 512>);

macro_rules! impl_paged_wrapper {
    ($wrapper:ident, $chunk:literal, $name:literal) => {
        impl<T> BenchCollection<T> for $wrapper<T> {
            fn new_empty() -> Self {
                Self(PagedSmallVec::with_layout())
            }

            fn push(&mut self, value: T) {
                self.0.push(value);
            }

            fn extend_from_slice(&mut self, values: &[T])
            where
                T: Clone,
            {
                self.0.extend_from_slice(values);
            }

            fn pop(&mut self) -> Option<T> {
                self.0.pop()
            }

            fn get(&self, index: usize) -> Option<&T> {
                self.0.get(index)
            }

            fn remove(&mut self, index: usize) -> Option<T> {
                self.0.remove(index)
            }

            fn swap_remove(&mut self, index: usize) -> Option<T> {
                self.0.swap_remove(index)
            }

            fn len(&self) -> usize {
                self.0.len()
            }

            fn for_each_ref(&self, f: impl FnMut(&T)) {
                self.0.for_each_ref(f)
            }

            fn name() -> &'static str {
                $name
            }
        }
    };
}

impl_paged_wrapper!(Paged128, 128, "PagedSmallVec<128>");
impl_paged_wrapper!(Paged256, 256, "PagedSmallVec<256>");
impl_paged_wrapper!(Paged512, 512, "PagedSmallVec<512>");

fn build_values<T: BenchValue>(n: usize) -> Vec<T> {
    (0..n).map(T::make).collect()
}

fn build_indices(len: usize, reads: usize) -> Vec<usize> {
    let mut state = 0x1234_5678usize;
    let mut out = Vec::with_capacity(reads);
    for _ in 0..reads {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push(state % len);
    }
    out
}

fn bench_append_only<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(LARGE_N);
    let mut group = c.benchmark_group(format!("append_only/{}", T::label()));
    group.throughput(Throughput::Elements(LARGE_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for value in &values {
                collection.push(black_box(value.clone()));
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_push_only_micro<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(PUSH_MICRO_N);
    let mut group = c.benchmark_group(format!("push_only_micro/{}", T::label()));
    group.throughput(Throughput::Elements(PUSH_MICRO_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for value in &values {
                collection.push(black_box(value.clone()));
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_extend_micro<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(PUSH_MICRO_N);
    let mut group = c.benchmark_group(format!("extend_micro/{}", T::label()));
    group.throughput(Throughput::Elements(PUSH_MICRO_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            collection.extend_from_slice(black_box(&values));
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_append_pop<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(LARGE_N);
    let mut group = c.benchmark_group(format!("append_pop/{}", T::label()));
    group.throughput(Throughput::Elements(LARGE_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for (i, value) in values.iter().enumerate() {
                collection.push(black_box(value.clone()));
                if i % POP_EVERY == 0 {
                    black_box(collection.pop());
                }
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_push_pop_micro<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(PUSH_MICRO_N);
    let mut group = c.benchmark_group(format!("push_pop_micro/{}", T::label()));
    group.throughput(Throughput::Elements(PUSH_MICRO_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for value in &values {
                collection.push(black_box(value.clone()));
            }
            for _ in 0..(PUSH_MICRO_N / 2) {
                black_box(collection.pop());
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_full_iteration<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(LARGE_N);
    let collection = C::build(&values);
    let mut group = c.benchmark_group(format!("full_iteration/{}", T::label()));
    group.throughput(Throughput::Elements(LARGE_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            collection.for_each_ref(|value| {
                sum = sum.wrapping_add(black_box(value).score());
            });
            black_box(sum)
        });
    });
    group.finish();
}

fn bench_random_indexing<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(LARGE_N);
    let indices = build_indices(LARGE_N, RANDOM_READS);
    let collection = C::build(&values);
    let mut group = c.benchmark_group(format!("random_indexing/{}", T::label()));
    group.throughput(Throughput::Elements(RANDOM_READS as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut sum = 0_u64;
            for &index in &indices {
                sum = sum.wrapping_add(black_box(collection.get(index).unwrap()).score());
            }
            black_box(sum)
        });
    });
    group.finish();
}

fn bench_remove<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(REMOVE_N);
    let mut group = c.benchmark_group(format!("remove/{}", T::label()));
    group.throughput(Throughput::Elements(REMOVE_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::build(&values);
            while collection.len() > 0 {
                let index = collection.len() / 2;
                black_box(collection.remove(index).unwrap());
            }
        });
    });
    group.finish();
}

fn bench_swap_remove<C, T>(c: &mut Criterion)
where
    C: BenchCollection<T>,
    T: BenchValue,
{
    let values = build_values::<T>(REMOVE_N);
    let mut group = c.benchmark_group(format!("swap_remove/{}", T::label()));
    group.throughput(Throughput::Elements(REMOVE_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::build(&values);
            while collection.len() > 0 {
                let index = collection.len() / 2;
                black_box(collection.swap_remove(index).unwrap());
            }
        });
    });
    group.finish();
}

fn bench_type<T: BenchValue>(c: &mut Criterion) {
    bench_append_only::<Vec<T>, T>(c);
    bench_append_only::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_append_only::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);

    bench_append_pop::<Vec<T>, T>(c);
    bench_append_pop::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_append_pop::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);

    bench_full_iteration::<Vec<T>, T>(c);
    bench_full_iteration::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_full_iteration::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);

    bench_random_indexing::<Vec<T>, T>(c);
    bench_random_indexing::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_random_indexing::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);

    bench_remove::<Vec<T>, T>(c);
    bench_remove::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_remove::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);

    bench_swap_remove::<Vec<T>, T>(c);
    bench_swap_remove::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_swap_remove::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT>, T>(c);
}

fn bench_push_path_type<T: BenchValue>(c: &mut Criterion) {
    bench_push_only_micro::<Vec<T>, T>(c);
    bench_push_only_micro::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_push_only_micro::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, PAGED_CHUNK>, T>(c);
    bench_push_only_micro::<Paged128<T>, T>(c);
    bench_push_only_micro::<Paged256<T>, T>(c);
    bench_push_only_micro::<Paged512<T>, T>(c);

    bench_extend_micro::<Vec<T>, T>(c);
    bench_extend_micro::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_extend_micro::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, PAGED_CHUNK>, T>(c);
    bench_extend_micro::<Paged128<T>, T>(c);
    bench_extend_micro::<Paged256<T>, T>(c);
    bench_extend_micro::<Paged512<T>, T>(c);

    bench_push_pop_micro::<Vec<T>, T>(c);
    bench_push_pop_micro::<SmallVec<[T; SMALLVEC_INLINE]>, T>(c);
    bench_push_pop_micro::<PagedSmallVec<T, PAGED_INLINE, PAGED_DIRECT, PAGED_CHUNK>, T>(c);
    bench_push_pop_micro::<Paged128<T>, T>(c);
    bench_push_pop_micro::<Paged256<T>, T>(c);
    bench_push_pop_micro::<Paged512<T>, T>(c);
}

fn compare(c: &mut Criterion) {
    bench_push_path_type::<u32>(c);
    bench_push_path_type::<[u8; 64]>(c);
    bench_push_path_type::<String>(c);

    bench_type::<u32>(c);
    bench_type::<[u8; 64]>(c);
    bench_type::<String>(c);
}

criterion_group!(benches, compare);
criterion_main!(benches);
