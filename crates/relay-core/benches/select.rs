//! P1: selection cost with 16 accounts.
use criterion::{Criterion, criterion_group, criterion_main};
use relay_core::select::{Candidate, Strategy, select};
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let cands: Vec<Candidate> = (0..16)
        .map(|i| Candidate {
            eligible: i % 5 != 0,
            headroom: Some((i * 7 % 100) as f32 / 100.0),
            resets_at: Some(1_000 + i as i64),
        })
        .collect();
    let mut cursor = 0usize;
    c.bench_function("select_quota_aware_16", |b| {
        b.iter(|| {
            cursor = cursor.wrapping_add(1);
            black_box(select(
                black_box(&cands),
                Strategy::QuotaAware,
                0.05,
                cursor,
            ))
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
