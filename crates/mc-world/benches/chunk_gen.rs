//! Benchmark: chunk generation performance for different generators.
use criterion::{black_box, Criterion, criterion_group, criterion_main};
use mc_core::position::ChunkPos;
use mc_world::generator::{NoiseGenerator, FlatGenerator, EmptyGenerator, TerrainGenerator};

fn bench_noise_generator(c: &mut Criterion) {
    let generator = NoiseGenerator::new();
    c.bench_function("chunk_gen_noise", |b| {
        b.iter(|| {
            let chunk = generator.generate_chunk(black_box(ChunkPos::new(0, 0)), black_box(12345));
            black_box(chunk);
        })
    });
}

fn bench_flat_generator(c: &mut Criterion) {
    let generator = FlatGenerator::default();
    c.bench_function("chunk_gen_flat", |b| {
        b.iter(|| {
            let chunk = generator.generate_chunk(black_box(ChunkPos::new(0, 0)), black_box(12345));
            black_box(chunk);
        })
    });
}

fn bench_empty_generator(c: &mut Criterion) {
    let generator = EmptyGenerator::new();
    c.bench_function("chunk_gen_empty", |b| {
        b.iter(|| {
            let chunk = generator.generate_chunk(black_box(ChunkPos::new(0, 0)), black_box(12345));
            black_box(chunk);
        })
    });
}

criterion_group!(benches, bench_noise_generator, bench_flat_generator, bench_empty_generator);
criterion_main!(benches);
