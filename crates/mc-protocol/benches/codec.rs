//! Benchmark: VarInt encode/decode performance.
use criterion::{black_box, Criterion, criterion_group, criterion_main};
use mc_protocol::varint;

fn bench_varint_write(c: &mut Criterion) {
    c.bench_function("varint_write_small", |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(5);
            varint::write_varint_to(black_box(127), &mut buf);
            black_box(buf);
        })
    });

    c.bench_function("varint_write_large", |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(5);
            varint::write_varint_to(black_box(2_147_483_647), &mut buf);
            black_box(buf);
        })
    });
}

fn bench_varint_read(c: &mut Criterion) {
    let small_encoded = {
        let mut buf = Vec::with_capacity(5);
        varint::write_varint_to(127, &mut buf);
        buf
    };
    let large_encoded = {
        let mut buf = Vec::with_capacity(5);
        varint::write_varint_to(2_147_483_647, &mut buf);
        buf
    };

    c.bench_function("varint_read_small", |b| {
        b.iter(|| {
            let (val, _) = varint::read_varint(black_box(&small_encoded)).unwrap();
            black_box(val);
        })
    });

    c.bench_function("varint_read_large", |b| {
        b.iter(|| {
            let (val, _) = varint::read_varint(black_box(&large_encoded)).unwrap();
            black_box(val);
        })
    });
}

criterion_group!(benches, bench_varint_write, bench_varint_read);
criterion_main!(benches);
