# paged-small-vec

An experimental Rust container that mixes inline storage with fixed-size heap chunks.

The design is intentionally inode-inspired at a high level: keep a small inline region, then spill into direct chunk pointers instead of growing one contiguous allocation forever. The point of this crate is not to beat `Vec` on normal workloads. It is to explore the trade-offs of a pointer-heavy, chunk-native layout and benchmark where it loses or occasionally starts to make sense.

## Layout

```rust
pub struct PagedSmallVec<
    T,
    const INLINE: usize = 10,
    const DIRECT: usize = 20,
    const CHUNK: usize = 256,
> {
    direct: [Option<Box<[MaybeUninit<T>; CHUNK]>>; DIRECT],
    inline: [MaybeUninit<T>; INLINE],
    len: usize,
    tail_chunk_index: usize,
    tail_offset: usize,
    current_chunk_ptr: *mut MaybeUninit<T>,
}
```

This gives the container:

- inline storage for small values
- paged growth without contiguous reallocation after spill
- chunk-native traversal APIs such as `chunks()`, `for_each_chunk()`, and `for_each_ref()`

## Current API

```rust
use paged_small_vec::PagedSmallVec;

let mut vec = PagedSmallVec::<u32, 32, 128, 256>::with_layout();
vec.push(1);
vec.push(2);
vec.extend_from_slice(&[3, 4, 5]);

assert_eq!(vec.get(2), Some(&3));
assert_eq!(vec.pop(), Some(5));
```

## Benchmarks

The repo includes Criterion benchmarks comparing:

- `Vec<T>`
- `SmallVec<[T; N]>`
- `PagedSmallVec<T, INLINE, DIRECT, CHUNK>`

Workloads:

- append-only
- append + occasional pop
- full iteration
- random indexing
- `remove`
- `swap_remove`
- focused push/pop/extend microbenches

Element types:

- `u32`
- `[u8; 64]`
- `String`

Run them with:

```bash
cargo bench --bench compare
cargo bench --bench push_u32
```

## What The Benchmarks Show

On normal vector-shaped workloads, `Vec` wins and `SmallVec` usually comes second. `PagedSmallVec` is most interesting when treated as a chunk-native structure rather than as a drop-in `Vec` competitor. Full sequential scans got much better once traversal was expressed chunk-by-chunk instead of through repeated indexing.

For the focused `u32` push-path benchmark, the current picture is roughly:

```text
push_only_micro/u32
Vec                 ~1.08 Gelem/s
SmallVec            ~0.77 Gelem/s
PagedSmallVec<256>  ~0.38 Gelem/s
PagedSmallVec<512>  ~0.34 Gelem/s

push_pop_micro/u32
Vec                 ~0.86 Gelem/s
SmallVec            ~0.59 Gelem/s
PagedSmallVec<256>  ~0.33 Gelem/s
PagedSmallVec<512>  ~0.31 Gelem/s

extend_micro/u32
Vec                 ~8.42 Gelem/s
SmallVec            ~10.54 Gelem/s
PagedSmallVec<256>  ~0.58 Gelem/s
PagedSmallVec<512>  ~0.61 Gelem/s
```

So the current verdict is simple: this is a useful experiment and an interesting chunked container, but it is not a better `Vec`.

## Safety

This crate uses `MaybeUninit<T>` internally and relies on manual invariants around initialized slots, logical length, and chunk state. The public unchecked accessor documents its safety contract, and the unsafe blocks in the implementation are annotated with `// SAFETY:` comments.

## License

MIT
