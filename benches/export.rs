//! Export benchmark for the binding generators.
//!
//! This is deliberately dependency-light: a fixed workload is exported in a loop while
//! `dhat` records allocations and bytes, so a change can be judged on wall time,
//! allocation count and allocation volume at once.
//!
//! ```sh
//! cargo bench --bench export --all-features
//! ```
//!
//! Every line of output has the shape `metric <name> <value>`. Compare two runs by
//! diffing those lines.

#![allow(missing_docs, unused, clippy::unwrap_used, clippy::panic)]

use std::{collections::HashMap, hint::black_box, path::PathBuf, time::Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::{JSDoc, Typescript};
use tauri::test::MockRuntime;
use tauri_specta::{
    Builder, Casing, ErrorHandlingMode, Event, LanguageExt, collect_commands, collect_events,
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// ----- workload types -----

/// A wide struct so type rendering, not just command plumbing, is measured.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct Wide {
    identifier: u32,
    display_name: String,
    description: String,
    tags: Vec<String>,
    scores: HashMap<String, i32>,
    homepage: Option<String>,
    avatar: Option<String>,
    created_at: String,
    updated_at: String,
    deleted: bool,
    verified: bool,
    followers: u64,
    following: u64,
    reputation: f64,
    location: Option<String>,
    language: Option<String>,
    flags: Vec<bool>,
    metadata: HashMap<String, String>,
    parent: Option<u32>,
    children: Vec<u32>,
}

#[derive(Serialize, Deserialize, Type)]
struct Point(f64, f64);

#[derive(Serialize, Deserialize, Type)]
struct Wrapper(String);

#[derive(Serialize, Deserialize, Type)]
#[serde(tag = "type", content = "data")]
enum EventKind {
    Unit,
    Tuple(String),
    Struct { id: u32 },
    Created { at: String, by: u32 },
    Updated {
        at: String,
        by: u32,
        fields: Vec<String>,
    },
    Deleted {
        at: String,
        by: u32,
    },
    Restored {
        at: String,
        by: u32,
    },
    Renamed {
        at: String,
        from: String,
        to: String,
    },
    Moved {
        at: String,
        parent: Option<u32>,
    },
    Tagged {
        at: String,
        tag: String,
    },
    Untagged {
        at: String,
        tag: String,
    },
    Archived {
        at: String,
    },
}

#[derive(Serialize, Deserialize, Type)]
#[serde(untagged)]
enum Scalar {
    Number(i32),
    Text(String),
    Flag(bool),
}

#[derive(Serialize, Deserialize, Type)]
struct Error1(#[specta(type = String)] String);

#[derive(Serialize, Deserialize, Type)]
enum Error2 {
    NotFound,
    Parse { line: u32 },
    Io(String),
}

#[derive(Serialize, Deserialize, Type, Event)]
struct MessageEvent {
    message: String,
}

#[derive(Serialize, Deserialize, Type, Event)]
struct WideEvent {
    event: EventKind,
    wide: Wide,
}

// ----- commands -----

#[tauri::command]
fn greet(my_name: String) -> String {
    my_name
}

#[tauri::command]
fn unit_command() {}

#[tauri::command(async)]
async fn async_command(delay_ms: u32) -> Wide {
    let _ = delay_ms;
    unreachable!()
}

#[tauri::command]
fn many_args(a: String, b: i32, c: bool, d: Vec<String>, e: Option<String>) -> bool {
    let _ = (a, b, d, e);
    c
}

#[tauri::command]
fn echo_wide(wide: Wide) -> Wide {
    wide
}

#[tauri::command]
fn tuple_types(point: Point) -> Wrapper {
    let _ = point;
    unreachable!()
}

#[tauri::command]
fn enums(kind: EventKind, scalar: Scalar) -> EventKind {
    let _ = scalar;
    kind
}

#[tauri::command]
fn plain_error() -> Result<String, Error1> {
    unreachable!()
}

#[tauri::command]
fn tagged_error() -> Result<Wide, Error2> {
    unreachable!()
}

#[tauri::command]
fn maybe() -> Option<Wide> {
    None
}

#[tauri::command]
fn with_channel(channel: tauri::ipc::Channel<i32>) {
    let _ = channel;
}

#[tauri::command]
fn skipped_args(state: tauri::State<'_, String>, visible: String) -> String {
    let _ = state;
    visible
}

// ----- benchmark -----

fn workload() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![
            greet,
            unit_command,
            async_command,
            many_args,
            echo_wide,
            tuple_types,
            enums,
            plain_error,
            tagged_error,
            maybe,
            skipped_args,
        ])
        .events(collect_events![MessageEvent, WideEvent])
        .typ::<EventKind>()
        .constant("APP_NAME", "Bench")
        .constant("MAX_RETRIES", 3)
        .error_handling(ErrorHandlingMode::Result)
        .dangerously_cast_bigints_to_number()
        .function_casing(Casing::CamelCase)
}

const TIME_ITERATIONS: usize = 200;
// Allocation tracking records a backtrace per allocation, so it is far too slow for the
// timing loop. Both phases cover the same workload; only the iteration counts differ.
const ALLOC_ITERATIONS: usize = 5;

fn bench_path(label: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tauri-specta-bench-{label}.{extension}"))
}

fn print_output_bytes(label: &str, path: &PathBuf) {
    println!(
        "metric {label}.output_bytes {}",
        std::fs::metadata(path).unwrap().len()
    );
    let _ = std::fs::remove_file(path);
}

/// Time a workload with allocation tracking disabled, so the numbers are not inflated by
/// dhat's per-allocation bookkeeping.
fn measure_time(label: &str, mut body: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..TIME_ITERATIONS {
        body();
    }
    let iter_ns = start.elapsed().as_nanos() as f64 / TIME_ITERATIONS as f64;
    println!("metric {label}.iter_ns {iter_ns:.1}");
}

/// Count allocations and bytes for the same workload. Requires a live dhat profiler.
fn measure_allocations(label: &str, mut body: impl FnMut()) {
    let before = dhat::HeapStats::get();
    for _ in 0..ALLOC_ITERATIONS {
        body();
    }
    let after = dhat::HeapStats::get();
    let blocks = (after.total_blocks - before.total_blocks) as f64 / ALLOC_ITERATIONS as f64;
    let bytes = (after.total_bytes - before.total_bytes) as f64 / ALLOC_ITERATIONS as f64;
    println!("metric {label}.allocs_per_iter {blocks:.1}");
    println!("metric {label}.bytes_per_iter {bytes:.1}");
}

fn bench_export<L: LanguageExt + Clone>(
    label: &str,
    builder: &Builder<MockRuntime>,
    language: L,
    extension: &str,
    profiler: &mut Option<dhat::Profiler>,
) where
    L::Error: std::fmt::Debug,
{
    let path = bench_path(label, extension);
    const WARMUP: usize = 5;

    for _ in 0..WARMUP {
        builder.export(language.clone(), &path).unwrap();
    }

    measure_time(label, || {
        builder.export(language.clone(), black_box(&path)).unwrap();
    });
    print_output_bytes(label, &path);

    // Allocation tracking is enabled only for the second pass over each workload.
    profiler.take();
    let tracking = dhat::Profiler::builder().testing().build();
    measure_allocations(label, || {
        builder.export(language.clone(), black_box(&path)).unwrap();
    });
    drop(tracking);
}

fn main() {
    let builder = workload();
    let mocks = builder.clone().mocks();
    let empty = Builder::<MockRuntime>::new();

    measure_time("collect", || {
        black_box(workload());
    });

    let mut profiler = None;
    bench_export("export_ts", &builder, Typescript::default(), "ts", &mut profiler);
    bench_export("export_jsdoc", &builder, JSDoc::default(), "js", &mut profiler);
    bench_export(
        "export_ts_mocks",
        &mocks,
        Typescript::default(),
        "ts",
        &mut profiler,
    );
    bench_export(
        "export_ts_empty",
        &empty,
        Typescript::default(),
        "ts",
        &mut profiler,
    );
}
