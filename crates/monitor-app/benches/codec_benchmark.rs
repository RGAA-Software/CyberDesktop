use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use monitor_app::monitor_codec::{decode_telemetry, encode_telemetry};
use monitor_app::sys_info::{SysInfo, SysProcessInfo};

fn sample_info() -> SysInfo {
    let mut info = SysInfo::default();
    for i in 0..256 {
        info.processes.push(SysProcessInfo {
            pid: i,
            name: format!("process_{}", i % 16),
            exe: "C:\\Windows\\System32\\notepad.exe".to_string(),
            command_line: format!("notepad.exe arg{}", i),
            cpu_usage: (i % 100) as f32,
            memory_mb: (i * 4) as u64,
            ..Default::default()
        });
    }
    info
}

fn bench_codec(c: &mut Criterion) {
    let info = sample_info();
    let encoded = encode_telemetry(&info).expect("encode");

    let mut group = c.benchmark_group("telemetry_codec");
    group.throughput(Throughput::Bytes(encoded.len() as u64));

    group.bench_function("encode", |b| {
        b.iter(|| {
            let bytes = encode_telemetry(black_box(&info)).expect("encode");
            black_box(bytes);
        })
    });

    group.bench_function("decode", |b| {
        b.iter(|| {
            let decoded = decode_telemetry(black_box(&encoded)).expect("decode");
            black_box(decoded);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_codec);
criterion_main!(benches);
