use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use paged_small_vec::PagedSmallVec;
use smallvec::SmallVec;

const SMALLVEC_INLINE: usize = 32;
const PAGED_INLINE: usize = 32;
const PAGED_DIRECT: usize = 128;
const PUSH_MICRO_N: usize = 4_096;

trait BenchCollection: Sized {
    fn new_empty() -> Self;
    fn push(&mut self, value: u32);
    fn extend_from_slice(&mut self, values: &[u32]);
    fn pop(&mut self) -> Option<u32>;
    fn len(&self) -> usize;
    fn name() -> &'static str;
}

impl BenchCollection for Vec<u32> {
    fn new_empty() -> Self {
        Vec::new()
    }

    fn push(&mut self, value: u32) {
        Vec::push(self, value);
    }

    fn extend_from_slice(&mut self, values: &[u32]) {
        self.extend_from_slice(values);
    }

    fn pop(&mut self) -> Option<u32> {
        Vec::pop(self)
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn name() -> &'static str {
        "Vec"
    }
}

impl BenchCollection for SmallVec<[u32; SMALLVEC_INLINE]> {
    fn new_empty() -> Self {
        SmallVec::new()
    }

    fn push(&mut self, value: u32) {
        SmallVec::push(self, value);
    }

    fn extend_from_slice(&mut self, values: &[u32]) {
        self.extend(values.iter().copied());
    }

    fn pop(&mut self) -> Option<u32> {
        SmallVec::pop(self)
    }

    fn len(&self) -> usize {
        SmallVec::len(self)
    }

    fn name() -> &'static str {
        "SmallVec"
    }
}

macro_rules! impl_paged_collection {
    ($chunk:literal, $name:literal) => {
        impl BenchCollection for PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, $chunk> {
            fn new_empty() -> Self {
                PagedSmallVec::with_layout()
            }

            fn push(&mut self, value: u32) {
                PagedSmallVec::push(self, value);
            }

            fn extend_from_slice(&mut self, values: &[u32]) {
                PagedSmallVec::extend_from_slice(self, values);
            }

            fn pop(&mut self) -> Option<u32> {
                PagedSmallVec::pop(self)
            }

            fn len(&self) -> usize {
                PagedSmallVec::len(self)
            }

            fn name() -> &'static str {
                $name
            }
        }
    };
}

impl_paged_collection!(256, "PagedSmallVec<256>");
impl_paged_collection!(512, "PagedSmallVec<512>");

fn build_values() -> Vec<u32> {
    (0..PUSH_MICRO_N as u32).collect()
}

fn bench_push_only<C: BenchCollection>(c: &mut Criterion) {
    let values = build_values();
    let mut group = c.benchmark_group("push_only_micro/u32");
    group.throughput(Throughput::Elements(PUSH_MICRO_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for &value in &values {
                collection.push(black_box(value));
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_push_pop<C: BenchCollection>(c: &mut Criterion) {
    let values = build_values();
    let mut group = c.benchmark_group("push_pop_micro/u32");
    group.throughput(Throughput::Elements(PUSH_MICRO_N as u64));
    group.bench_function(C::name(), |b| {
        b.iter(|| {
            let mut collection = C::new_empty();
            for &value in &values {
                collection.push(black_box(value));
            }
            for _ in 0..(PUSH_MICRO_N / 2) {
                black_box(collection.pop());
            }
            black_box(collection.len())
        });
    });
    group.finish();
}

fn bench_extend<C: BenchCollection>(c: &mut Criterion) {
    let values = build_values();
    let mut group = c.benchmark_group("extend_micro/u32");
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

fn push_u32(c: &mut Criterion) {
    bench_push_only::<Vec<u32>>(c);
    bench_push_only::<SmallVec<[u32; SMALLVEC_INLINE]>>(c);
    bench_push_only::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 256>>(c);
    bench_push_only::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 512>>(c);

    bench_push_pop::<Vec<u32>>(c);
    bench_push_pop::<SmallVec<[u32; SMALLVEC_INLINE]>>(c);
    bench_push_pop::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 256>>(c);
    bench_push_pop::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 512>>(c);

    bench_extend::<Vec<u32>>(c);
    bench_extend::<SmallVec<[u32; SMALLVEC_INLINE]>>(c);
    bench_extend::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 256>>(c);
    bench_extend::<PagedSmallVec<u32, PAGED_INLINE, PAGED_DIRECT, 512>>(c);
}

criterion_group!(benches, push_u32);
criterion_main!(benches);
