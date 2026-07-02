//! `cyber_files --shell-menu-test` — CLI diagnostic for Shell context-menu enumeration.
//!
//! Runs the exact same query pipeline the app uses on right-click, prints every stage
//! with timings to stdout/stderr, and exits without launching the GUI. Intended for
//! reproducing "menu hangs / incomplete list" reports on end-user machines.
//!
//! Usage:
//!   cyber_files --shell-menu-test [PATH ...] [--extended] [--no-layer-b] [--repeat N]
//!
//! Defaults: PATH = user Desktop; full pipeline (Layer A aggregate + Layer B per-handler).
//! `--no-layer-b` runs only the aggregate Shell menu (the Files.app-equivalent path).
//! Note: the GUI app now runs Layer A only by default; this diagnostic still defaults
//! to the full pipeline so hangs caused by Layer B remain reproducible.
//!
//! Exit codes: 0 = entries returned; 2 = query returned empty (likely timeout); 1 = error.

use std::path::PathBuf;
use std::time::Instant;

use app_platform_windows as platform;

struct TestConfig {
    paths: Vec<PathBuf>,
    extended: bool,
    layer_b: bool,
    repeat: u32,
}

fn parse_args(args: &[String]) -> Result<TestConfig, String> {
    let mut paths = Vec::new();
    let mut extended = false;
    let mut layer_b = true;
    let mut repeat = 1u32;

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--shell-menu-test" => {}
            "--extended" => extended = true,
            "--no-layer-b" => layer_b = false,
            "--repeat" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--repeat requires a number".to_string())?;
                repeat = value
                    .parse()
                    .map_err(|_| format!("invalid --repeat value: {value}"))?;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag: {other}"));
            }
            path => paths.push(PathBuf::from(path)),
        }
    }

    if paths.is_empty() {
        paths.push(default_test_path());
    }

    Ok(TestConfig {
        paths,
        extended,
        layer_b,
        repeat: repeat.max(1),
    })
}

fn default_test_path() -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let desktop = PathBuf::from(&profile).join("Desktop");
        if desktop.is_dir() {
            return desktop;
        }
        return PathBuf::from(profile);
    }
    std::env::temp_dir()
}

fn print_entries(entries: &[platform::ShellContextMenuEntry], indent: usize) {
    let pad = "  ".repeat(indent);
    for entry in entries {
        match entry {
            platform::ShellContextMenuEntry::Separator => {
                println!("{pad}----------------");
            }
            platform::ShellContextMenuEntry::Item {
                label,
                command_offset,
                command_string,
                icon_png,
                handler_clsid,
            } => {
                println!(
                    "{pad}{label}  [offset={command_offset} verb={} icon={} layer={}]",
                    command_string.as_deref().unwrap_or("-"),
                    if icon_png.as_ref().is_some_and(|p| !p.is_empty()) {
                        "yes"
                    } else {
                        "no"
                    },
                    if handler_clsid.is_some() { "B" } else { "A" },
                );
            }
            platform::ShellContextMenuEntry::Submenu {
                label,
                children,
                lazy_parent_index,
                handler_clsid,
                ..
            } => {
                println!(
                    "{pad}{label}/  [children={} lazy={} layer={}]",
                    children.len(),
                    lazy_parent_index.is_some(),
                    if handler_clsid.is_some() { "B" } else { "A" },
                );
                print_entries(children, indent + 1);
            }
        }
    }
}

fn count_entries(entries: &[platform::ShellContextMenuEntry]) -> usize {
    entries
        .iter()
        .map(|entry| match entry {
            platform::ShellContextMenuEntry::Submenu { children, .. } => {
                1 + count_entries(children)
            }
            _ => 1,
        })
        .sum()
}

pub fn run(args: &[String]) -> i32 {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!(
                "usage: cyber_files --shell-menu-test [PATH ...] [--extended] [--no-layer-b] [--repeat N]"
            );
            return 1;
        }
    };

    platform::set_shell_menu_layer_b_enabled(config.layer_b);
    let icon_px = platform::menu_icon_pixel_size(platform::system_scale_factor());

    println!("=== cyber_files shell menu diagnostic ===");
    println!("paths      : {:?}", config.paths);
    println!("extended   : {}", config.extended);
    println!(
        "pipeline   : {}",
        if config.layer_b {
            "Layer A (aggregate) + Layer B (per-handler probes)"
        } else {
            "Layer A (aggregate) only — Files.app-equivalent"
        }
    );
    println!("icon_px    : {icon_px}");
    println!("repeat     : {}", config.repeat);
    println!();

    let mut worst_exit = 0i32;
    for round in 1..=config.repeat {
        println!("--- round {round}/{} ---", config.repeat);
        let start = Instant::now();
        let result =
            platform::query_shell_context_menu_items(&config.paths, config.extended, icon_px);
        let elapsed = start.elapsed();

        match result {
            Ok(entries) if entries.is_empty() => {
                println!(
                    "round {round}: EMPTY after {elapsed:?} — likely QueryContextMenu timed out \
                     (check shell_menu warnings above for the wedged stage)"
                );
                worst_exit = worst_exit.max(2);
            }
            Ok(entries) => {
                println!(
                    "round {round}: OK in {elapsed:?} — {} top-level entries, {} total",
                    entries.len(),
                    count_entries(&entries)
                );
                print_entries(&entries, 1);
            }
            Err(error) => {
                println!("round {round}: ERROR after {elapsed:?} — {error:#}");
                worst_exit = worst_exit.max(1);
            }
        }

        let release_start = Instant::now();
        platform::clear_shell_menu_session();
        println!("round {round}: session released in {:?}", release_start.elapsed());
        println!();
    }

    println!("=== diagnostic done (exit code {worst_exit}) ===");
    worst_exit
}
