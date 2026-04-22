use std::{env, hint::black_box, mem::size_of};

use paged_small_vec::PagedSmallVec;
use smallvec::SmallVec;

const INLINE: usize = 32;
const DIRECT: usize = 4096;
const CHUNK_256: usize = 256;
const CHUNK_512: usize = 512;

fn build_vec(n: usize) -> usize {
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i as u32);
    }
    black_box(vec.len())
}

fn build_smallvec(n: usize) -> usize {
    let mut vec = SmallVec::<[u32; INLINE]>::new();
    for i in 0..n {
        vec.push(i as u32);
    }
    black_box(vec.len())
}

fn build_paged_256(n: usize) -> usize {
    let mut vec = PagedSmallVec::<u32, INLINE, DIRECT, CHUNK_256>::with_layout();
    for i in 0..n {
        vec.push(i as u32);
    }
    black_box(vec.len())
}

fn build_paged_512(n: usize) -> usize {
    let mut vec = PagedSmallVec::<u32, INLINE, DIRECT, CHUNK_512>::with_layout();
    for i in 0..n {
        vec.push(i as u32);
    }
    black_box(vec.len())
}

fn main() {
    let mut args = env::args().skip(1);
    let kind = args.next().expect("usage: memory_profile <kind> <n>");
    let n = args
        .next()
        .expect("usage: memory_profile <kind> <n>")
        .parse::<usize>()
        .expect("n must be usize");

    match kind.as_str() {
        "vec" => {
            println!("container=Vec<u32>");
            println!("stack_bytes={}", size_of::<Vec<u32>>());
            println!("len={}", build_vec(n));
        }
        "smallvec" => {
            println!("container=SmallVec<[u32; 32]>");
            println!("stack_bytes={}", size_of::<SmallVec<[u32; INLINE]>>());
            println!("len={}", build_smallvec(n));
        }
        "paged256" => {
            println!("container=PagedSmallVec<u32, 32, 4096, 256>");
            println!(
                "stack_bytes={}",
                size_of::<PagedSmallVec<u32, INLINE, DIRECT, CHUNK_256>>()
            );
            println!("len={}", build_paged_256(n));
        }
        "paged512" => {
            println!("container=PagedSmallVec<u32, 32, 4096, 512>");
            println!(
                "stack_bytes={}",
                size_of::<PagedSmallVec<u32, INLINE, DIRECT, CHUNK_512>>()
            );
            println!("len={}", build_paged_512(n));
        }
        _ => panic!("unknown kind"),
    }
}
