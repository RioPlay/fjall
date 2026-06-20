// Dependency-free microbenchmarks isolating the two data-structure changes from
// PR #301 (journal eviction queue, batch stall-check set). These do NOT use any
// fjall internals — they replicate the exact operation patterns so the old and
// new structures can be compared head-to-head in one binary.
//
// Reference artifact only; not part of the PR. Run with:
//   cargo bench --bench hotpath_micro
//
// See benches/README.md for methodology and recorded numbers.

use std::collections::{HashSet, VecDeque};
use std::hint::black_box;
use std::time::Instant;

// Roughly the size/shape of journal::manager::Item (PathBuf + u64 + Vec).
#[derive(Clone)]
struct Item {
    path: String,
    size_in_bytes: u64,
    _watermarks: Vec<u64>,
}

fn make_items(n: usize) -> Vec<Item> {
    (0..n)
        .map(|i| Item {
            path: format!("/var/lib/fjall/journals/{i}.jnl"),
            size_in_bytes: i as u64,
            _watermarks: Vec::new(),
        })
        .collect()
}

/// Best-of-`iters` wall time in microseconds (best-of cuts scheduler noise).
fn bench<F: FnMut()>(iters: u32, mut f: F) -> f64 {
    f(); // warmup
    let mut best = f64::MAX;
    for _ in 0..iters {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    best * 1e6
}

// Mirrors OLD JournalManager::maintenance: repeatedly remove(0) -> O(n) shift each.
fn drain_vec(n: usize) {
    let mut v = make_items(n);
    let mut acc = 0u64;
    while !v.is_empty() {
        acc = acc.wrapping_add(v[0].size_in_bytes);
        v.remove(0);
    }
    black_box(acc);
}

// Mirrors NEW JournalManager::maintenance: pop_front -> O(1).
fn drain_deque(n: usize) {
    let mut v: VecDeque<Item> = make_items(n).into();
    let mut acc = 0u64;
    while let Some(front) = v.front() {
        acc = acc.wrapping_add(front.size_in_bytes);
        v.pop_front();
    }
    black_box(acc);
}

// Stall-set: M batch items referencing K distinct keyspaces (deduped, then iterated).
fn stall_hashset(m: usize, k: usize) {
    let mut set: HashSet<String> = HashSet::new();
    for i in 0..m {
        set.insert(format!("keyspace_{}", i % k));
    }
    let mut acc = 0usize;
    for ks in &set {
        acc = acc.wrapping_add(ks.len());
    }
    black_box(acc);
}

fn stall_vec(m: usize, k: usize) {
    let mut v: Vec<String> = Vec::new();
    for i in 0..m {
        let name = format!("keyspace_{}", i % k);
        if !v.contains(&name) {
            v.push(name);
        }
    }
    let mut acc = 0usize;
    for ks in &v {
        acc = acc.wrapping_add(ks.len());
    }
    black_box(acc);
}

fn main() {
    println!("== Journal eviction drain: Vec::remove(0) vs VecDeque::pop_front ==");
    println!(
        "{:>8} | {:>14} | {:>14} | {:>8}",
        "N", "Vec (us)", "VecDeque (us)", "speedup"
    );
    for &n in &[16usize, 64, 256, 1024, 4096, 16384] {
        let iters = if n >= 4096 { 20 } else { 200 };
        let tv = bench(iters, || drain_vec(black_box(n)));
        let td = bench(iters, || drain_deque(black_box(n)));
        println!("{n:>8} | {tv:>14.3} | {td:>14.3} | {:>7.2}x", tv / td);
    }

    println!();
    println!("== Stall-set: HashSet vs Vec dedup+iterate (M items, K distinct keyspaces) ==");
    println!(
        "{:>10} | {:>14} | {:>14} | {:>8}",
        "M/K", "HashSet (us)", "Vec (us)", "speedup"
    );
    for &(m, k) in &[
        (4usize, 1usize),
        (8, 2),
        (16, 4),
        (64, 8),
        (256, 16),
        (1000, 64),
    ] {
        let th = bench(2000, || stall_hashset(black_box(m), black_box(k)));
        let tv = bench(2000, || stall_vec(black_box(m), black_box(k)));
        println!(
            "{m:>6}/{k:<3} | {th:>14.4} | {tv:>14.4} | {:>7.2}x",
            th / tv
        );
    }
}
