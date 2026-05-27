//! Compare serialize/deserialize size and time for typical AppConfig payloads.
//! Run: `cargo run -p cyberfiles-core --features config_bench --bin config_formats_bench --release`

use std::hint::black_box;
use std::time::Instant;

use cyberfiles_core::AppConfig;

fn sample_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.locale = "zh-CN".into();
    config.theme_name = "One".into();
    config.pinned_folders = (0..12)
        .map(|i| format!(r"C:\Users\demo\Documents\folder_{i}"))
        .collect();
    config.path_history = (0..80)
        .map(|i| format!(r"D:\projects\repo_{i}\src\main.rs"))
        .collect();
    config.session_tabs = vec![
        "home".into(),
        r"D:\source\CyberFiles".into(),
        r"C:\Users\demo\Downloads".into(),
    ];
    config.session_active_tab = 1;
    config.file_tags = (0..5)
        .map(|i| cyberfiles_core::FileTagConfig {
            name: format!("tag-{i}"),
            color: Some("#ff5500".into()),
            paths: (0..8)
                .map(|j| format!(r"C:\tag\{i}\file_{j}.txt"))
                .collect(),
        })
        .collect();
    config
}

fn main() {
    let config = sample_config();
    let rounds = 500u32;

    let start = Instant::now();
    for _ in 0..rounds {
        let s = serde_json::to_string_pretty(&config).unwrap();
        black_box(&s);
    }
    let json_pretty_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    for _ in 0..rounds {
        let s = serde_json::to_string(&config).unwrap();
        black_box(serde_json::from_str::<AppConfig>(&s).unwrap());
    }
    let json_roundtrip_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    let mut ron_len = 0usize;
    for _ in 0..rounds {
        let s = ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default()).unwrap();
        ron_len = s.len();
        black_box(&s);
    }
    let ron_pretty_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    for _ in 0..rounds {
        let s = ron::ser::to_string(&config).unwrap();
        black_box(ron::de::from_str::<AppConfig>(&s).unwrap());
    }
    let ron_roundtrip_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    let mut bincode_len = 0usize;
    for _ in 0..rounds {
        let bytes = bincode::serialize(&config).unwrap();
        bincode_len = bytes.len();
        black_box(&bytes);
    }
    let bincode_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    for _ in 0..rounds {
        let bytes = bincode::serialize(&config).unwrap();
        black_box(bincode::deserialize::<AppConfig>(&bytes).unwrap());
    }
    let bincode_roundtrip_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    let mut postcard_len = 0usize;
    for _ in 0..rounds {
        let bytes = postcard::to_allocvec(&config).unwrap();
        postcard_len = bytes.len();
        black_box(&bytes);
    }
    let postcard_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let start = Instant::now();
    for _ in 0..rounds {
        let bytes = postcard::to_allocvec(&config).unwrap();
        black_box(postcard::from_bytes::<AppConfig>(&bytes).unwrap());
    }
    let postcard_roundtrip_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

    let json_pretty_once = serde_json::to_string_pretty(&config).unwrap();
    let json_compact_once = serde_json::to_string(&config).unwrap();

    println!("CyberFiles AppConfig benchmark ({rounds} rounds, representative payload)");
    println!();
    println!("Payload size (bytes):");
    println!("  JSON pretty (current):  {}", json_pretty_once.len());
    println!("  JSON compact:           {}", json_compact_once.len());
    println!("  RON pretty:             {ron_len}");
    println!("  bincode:                {bincode_len}");
    println!("  postcard:               {postcard_len}");
    println!();
    println!("Serialize mean (ms/op):");
    println!("  JSON pretty:            {json_pretty_ms:.4}");
    println!("  RON pretty:             {ron_pretty_ms:.4}");
    println!("  bincode:                {bincode_ms:.4}");
    println!("  postcard:               {postcard_ms:.4}");
    println!();
    println!("Round-trip deserialize mean (ms/op):");
    println!("  JSON:                   {json_roundtrip_ms:.4}");
    println!("  RON:                    {ron_roundtrip_ms:.4}");
    println!("  bincode:                {bincode_roundtrip_ms:.4}");
    println!("  postcard:               {postcard_roundtrip_ms:.4}");
}
