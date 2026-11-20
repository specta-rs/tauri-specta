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

// ----- wide collect workload -----
//
// `collect` covers the shape of a small app. Real apps register dozens of commands,
// so `collect_wide` is the representative metric for registration cost: 50 commands
// (including five-argument and async signatures) and 10 events.

macro_rules! five_arg_commands {
    ($($name:ident),* $(,)?) => {
        $(
            #[tauri::command]
            fn $name(a: String, b: i32, c: bool, d: Vec<String>, e: Option<String>) -> bool {
                let _ = (a, b, d, e);
                c
            }
        )*
    };
}

#[rustfmt::skip]
five_arg_commands!(
    wide_cmd_00, wide_cmd_01, wide_cmd_02, wide_cmd_03, wide_cmd_04, wide_cmd_05, wide_cmd_06,
    wide_cmd_07, wide_cmd_08, wide_cmd_09, wide_cmd_10, wide_cmd_11, wide_cmd_12, wide_cmd_13,
    wide_cmd_14, wide_cmd_15, wide_cmd_16, wide_cmd_17, wide_cmd_18, wide_cmd_19, wide_cmd_20,
    wide_cmd_21, wide_cmd_22, wide_cmd_23, wide_cmd_24, wide_cmd_25, wide_cmd_26, wide_cmd_27,
    wide_cmd_28, wide_cmd_29, wide_cmd_30, wide_cmd_31, wide_cmd_32, wide_cmd_33, wide_cmd_34,
    wide_cmd_35, wide_cmd_36, wide_cmd_37, wide_cmd_38, wide_cmd_39, wide_cmd_40, wide_cmd_41,
    wide_cmd_42, wide_cmd_43, wide_cmd_44,
);

#[tauri::command]
fn wide_async(delay_ms: u32, wide: Wide) -> Wide {
    let _ = delay_ms;
    wide
}

#[tauri::command]
fn wide_echo(wide: Wide) -> Wide {
    wide
}

#[tauri::command]
fn wide_unit() {}

#[tauri::command]
fn wide_with_channel(channel: tauri::ipc::Channel<i32>, label: String) {
    let _ = (channel, label);
}

#[tauri::command]
fn wide_error() -> Result<Wide, Error2> {
    unreachable!()
}

macro_rules! wide_events {
    ($($name:ident),* $(,)?) => {
        $(
            #[derive(Serialize, Deserialize, Type, Event)]
            struct $name {
                id: u32,
                label: String,
            }
        )*
    };
}

#[rustfmt::skip]
wide_events!(
    WideEvent00, WideEvent01, WideEvent02, WideEvent03, WideEvent04, WideEvent05, WideEvent06,
    WideEvent07, WideEvent08, WideEvent09,
);

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

fn workload_wide() -> Builder<MockRuntime> {
    Builder::<MockRuntime>::new()
        .commands(collect_commands![
            wide_cmd_00,
            wide_cmd_01,
            wide_cmd_02,
            wide_cmd_03,
            wide_cmd_04,
            wide_cmd_05,
            wide_cmd_06,
            wide_cmd_07,
            wide_cmd_08,
            wide_cmd_09,
            wide_cmd_10,
            wide_cmd_11,
            wide_cmd_12,
            wide_cmd_13,
            wide_cmd_14,
            wide_cmd_15,
            wide_cmd_16,
            wide_cmd_17,
            wide_cmd_18,
            wide_cmd_19,
            wide_cmd_20,
            wide_cmd_21,
            wide_cmd_22,
            wide_cmd_23,
            wide_cmd_24,
            wide_cmd_25,
            wide_cmd_26,
            wide_cmd_27,
            wide_cmd_28,
            wide_cmd_29,
            wide_cmd_30,
            wide_cmd_31,
            wide_cmd_32,
            wide_cmd_33,
            wide_cmd_34,
            wide_cmd_35,
            wide_cmd_36,
            wide_cmd_37,
            wide_cmd_38,
            wide_cmd_39,
            wide_cmd_40,
            wide_cmd_41,
            wide_cmd_42,
            wide_cmd_43,
            wide_cmd_44,
            wide_async,
            wide_echo,
            wide_unit,
            wide_with_channel,
            wide_error,
        ])
        .events(collect_events![
            WideEvent00,
            WideEvent01,
            WideEvent02,
            WideEvent03,
            WideEvent04,
            WideEvent05,
            WideEvent06,
            WideEvent07,
            WideEvent08,
            WideEvent09,
        ])
        .typ::<EventKind>()
        .constant("APP_NAME", "Bench")
        .constant("MAX_RETRIES", 3)
        .error_handling(ErrorHandlingMode::Result)
        .dangerously_cast_bigints_to_number()
        .function_casing(Casing::CamelCase)
}

const TIME_ITERATIONS: usize = 200;
// Timing is reported as the fastest of several batches. Interference from other work on
// the machine only ever makes a batch slower, so the minimum is a more stable estimate
// of the workload's own cost than the mean of one long run.
const TIME_BATCHES: usize = 5;
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
fn measure_time(label: &str, iterations: usize, mut body: impl FnMut()) {
    let mut best = f64::INFINITY;
    for _ in 0..TIME_BATCHES {
        let start = Instant::now();
        for _ in 0..iterations {
            body();
        }
        let iter_ns = start.elapsed().as_nanos() as f64 / iterations as f64;
        best = best.min(iter_ns);
    }
    println!("metric {label}.iter_ns {best:.1}");
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

/// Measure a builder-construction workload: wall time, then allocation count and bytes.
fn bench_collect(label: &str, mut workload: impl FnMut() -> Builder<MockRuntime>) {
    // Registration is cheap, so it needs more iterations than an export to fill a batch.
    measure_time(label, TIME_ITERATIONS * 5, || {
        black_box(workload());
    });

    let tracking = dhat::Profiler::builder().testing().build();
    measure_allocations(label, || {
        black_box(workload());
    });
    drop(tracking);
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

    measure_time(label, TIME_ITERATIONS, || {
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

    bench_collect("collect", workload);
    bench_collect("collect_wide", workload_wide);

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
