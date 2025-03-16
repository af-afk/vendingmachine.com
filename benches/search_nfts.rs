#![cfg(not(target_arch = "wasm32"))]

use stylus_sdk::{alloy_primitives::U256, host::VM, prelude::StorageType, testing::vm::TestVM};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use libvendingmachine::*;

fn crit_pick_levels(crit: &mut Criterion) {
    let mut c = unsafe {
        StorageVendingMachine::new(
            U256::ZERO,
            0,
            VM {
                host: Box::new(TestVM::new()),
            },
        )
    };
    // Simple benchmarking test without accommodating different ranges of NFT levels.
    let max = 1_000_000;
    for i in 0..max {
        let mut l = c.levels.grow();
        l.usd_min.set(U256::from(i));
        l.nfts_distributeable.grow();
    }
    let mut g = crit.benchmark_group("Level selection (basic)");
    g.bench_function("Core", |_| {
        black_box(c.pick_level(U256::from((max / 2) - 100)).unwrap());
    });
    g.bench_function("Manual", |_| {
        black_box(c.pick_level_manual(U256::from((max / 2) - 100)).unwrap());
        panic!("shit");
    });
    g.finish();
}

criterion_group!(benches, crit_pick_levels);
criterion_main!(benches);
