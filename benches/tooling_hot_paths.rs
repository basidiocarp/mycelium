use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mycelium::{classify_command, rewrite_command};

fn bench_classify_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("discover::classify_command");

    for command in [
        "git status",
        "cargo test -- --nocapture",
        "pnpm exec playwright test",
        "git status && cargo test",
        "ansible-playbook site.yml",
    ] {
        group.bench_function(BenchmarkId::new("classify", command), |b| {
            b.iter(|| black_box(classify_command(black_box(command))))
        });
    }

    group.finish();
}

fn bench_rewrite_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("discover::rewrite_command");
    let excluded: Vec<String> = Vec::new();

    for command in [
        "git status",
        "cargo test -- --nocapture",
        "git status && cargo test",
        "git log -10 | grep commit",
    ] {
        group.bench_function(BenchmarkId::new("rewrite", command), |b| {
            b.iter(|| black_box(rewrite_command(black_box(command), black_box(&excluded))))
        });
    }

    group.finish();
}

criterion_group!(tooling_hot_paths, bench_classify_command, bench_rewrite_command);
criterion_main!(tooling_hot_paths);
