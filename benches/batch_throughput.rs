// End-to-end batch-commit throughput through fjall's public API. Exercises the
// batch-commit hot path touched by PR #301 (changes 2 and 3). Reports throughput
// for the CURRENT checkout; to get a before/after, run this on `main` and on the
// PR branch (or use a git worktree) and compare.
//
// Reference artifact only; not part of the PR. Run with:
//   cargo bench --bench batch_throughput
//
// See benches/README.md for methodology and recorded before/after numbers.

use fjall::{Database, KeyspaceCreateOptions};
use std::time::Instant;

fn main() -> fjall::Result<()> {
    // A: single-thread, single keyspace, many small batches.
    {
        let dir = tempfile::tempdir()?;
        let db = Database::builder(dir.path()).open()?;
        let ks = db.keyspace("default", KeyspaceCreateOptions::default)?;

        let n_batches = 20_000;
        let items_per_batch = 8;

        let t = Instant::now();
        for b in 0..n_batches {
            let mut batch = db.batch();
            for i in 0..items_per_batch {
                batch.insert(&ks, format!("k-{b}-{i}"), "value-payload-xyz");
            }
            batch.commit()?;
        }
        let secs = t.elapsed().as_secs_f64();
        println!(
            "A single-thread 1ks : {secs:.3}s  {:.0} commits/s  {:.0} items/s",
            n_batches as f64 / secs,
            (n_batches * items_per_batch) as f64 / secs,
        );
    }

    // B: single-thread, multi-keyspace batches (exercises stall-set dedup).
    {
        let dir = tempfile::tempdir()?;
        let db = Database::builder(dir.path()).open()?;
        let kss: Vec<_> = (0..6)
            .map(|i| db.keyspace(&format!("ks{i}"), KeyspaceCreateOptions::default))
            .collect::<Result<_, _>>()?;

        let n_batches = 20_000;
        let t = Instant::now();
        for b in 0..n_batches {
            let mut batch = db.batch();
            for i in 0..12 {
                batch.insert(
                    &kss[i % kss.len()],
                    format!("k-{b}-{i}"),
                    "value-payload-xyz",
                );
            }
            batch.commit()?;
        }
        let secs = t.elapsed().as_secs_f64();
        println!(
            "B single-thread 6ks : {secs:.3}s  {:.0} commits/s",
            n_batches as f64 / secs
        );
    }

    // C: concurrent committers (write contention).
    {
        let dir = tempfile::tempdir()?;
        let db = Database::builder(dir.path()).open()?;
        let ks = db.keyspace("default", KeyspaceCreateOptions::default)?;

        let threads = 4;
        let per_thread = 10_000;
        let t = Instant::now();
        std::thread::scope(|s| {
            for tid in 0..threads {
                let db = db.clone();
                let ks = ks.clone();
                s.spawn(move || {
                    for b in 0..per_thread {
                        let mut batch = db.batch();
                        for i in 0..8 {
                            batch.insert(&ks, format!("k-{tid}-{b}-{i}"), "value-payload-xyz");
                        }
                        batch.commit().unwrap();
                    }
                });
            }
        });
        let secs = t.elapsed().as_secs_f64();
        println!(
            "C concurrent x{threads}    : {secs:.3}s  {:.0} commits/s",
            (threads * per_thread) as f64 / secs,
        );
    }

    Ok(())
}
