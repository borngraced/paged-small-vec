# paged-small-vec

An experimental Rust container that mixes inline storage with fixed-size heap chunks.

The design is intentionally inode-inspired at a high level: keep a small inline region, then spill into direct chunk pointers instead of growing one contiguous allocation forever. The point of this crate is not to beat `Vec` on normal workloads. It is to explore the trade-offs of a pointer-heavy, chunk-native layout and benchmark where it loses or occasionally starts to make sense.

https://sot.dev/inode-style-vector-in-rust.html

## License

MIT
