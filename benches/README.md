# Hot-path microbenchmarks (reference)

These benchmarks back the performance discussion in **PR #301**
(structural hot-path cleanups: journal eviction queue + batch-commit set).

They live on this branch (`bench/hotpath-microbenches`) **only** — they are
deliberately *not* part of PR #301, to keep that diff focused on the three
changes + tests. They are here so the numbers are reproducible and viewable.

Both are dependency-free (`std::time` + `std::hint::black_box`, no criterion)
and wired with `harness = false`, so each is just a plain binary.

## Running

```sh
cargo bench --bench hotpath_micro      # isolated data-structure comparison
cargo bench --bench batch_throughput   # end-to-end batch commit (current checkout)
```

For a real before/after on `batch_throughput`, run it on `main` and on the PR
branch (or via `git worktree add /tmp/fjall-main <main-sha>`) and compare.

## Methodology / honesty notes

- Microbenchmarks (`hotpath_micro`) are **best-of-N** to cut scheduler noise.
- End-to-end (`batch_throughput`) is **raw repeated runs**; concurrent results
  (scenario C) are noisy — treat as directional, not precise.
- These are **not** criterion-grade statistics (no warmup modeling, no outlier
  analysis, no confidence intervals). They are directional and reproducible.
- `hotpath_micro` does not touch fjall internals; it replicates the exact
  operation patterns so old (`Vec`) and new (`VecDeque`/`Vec`) can be compared
  head-to-head in one binary.

## Recorded numbers

Machine: Intel Core Ultra 9 285H, `rustc 1.95.0`, `--release` / `opt-level = 3`.

### Journal eviction drain: `Vec::remove(0)` vs `VecDeque::pop_front`

| N (queue len)        | Vec (us) | VecDeque (us) | speedup |
|---------------------:|---------:|--------------:|--------:|
| 16 (realistic)       |   0.461  |      0.399    |  1.16x  |
| 256                  |  22.96   |      8.03     |  2.86x  |
| 1024                 | 167.3    |     22.1      |  7.58x  |
| 16384 (pathological) | 78138    |    644        |  121x   |

Real queue length is a handful, so the N=16 row is the honest one: a
sub-microsecond, negligible difference. The large speedups are purely
asymptotic and should not occur in practice.

### Stall-set dedup+iterate: `HashSet` vs `Vec` (M items / K distinct keyspaces)

| M / K     | HashSet (us) | Vec (us) | speedup            |
|----------:|-------------:|---------:|--------------------|
| 4 / 1     |   0.117      |  0.067   | 1.75x              |
| 16 / 4    |   0.458      |  0.273   | 1.68x              |
| 64 / 8    |   1.827      |  1.166   | 1.57x              |
| 256 / 16  |   7.05       |  5.06    | 1.39x              |
| 1000 / 64 |  28.3        | 50.6     | 0.56x (Vec slower) |

`Vec` + linear `contains()` is O(M*K): a win for the common small case, a
regression once a batch touches many distinct keyspaces (crossover ~64).

### End-to-end batch-commit throughput (before = `main`, after = PR branch)

| Scenario                                  | before (commits/s) | after (commits/s) | verdict                  |
|-------------------------------------------|-------------------:|------------------:|--------------------------|
| A: single-thread, 1 keyspace, 8 items     |     ~281k–283k     |    ~289k–301k     | small, consistent ~5%    |
| B: single-thread, 6 keyspaces, 12 items   |     ~204k–207k     |    ~199k–210k     | within noise             |
| C: 4 concurrent committers                |     ~137k–144k     |    ~127k–209k     | too noisy to conclude    |

Net: no regression; a small win in the single-keyspace case, consistent with
removing the per-commit `HashSet` allocation. Not a dramatic speedup.
