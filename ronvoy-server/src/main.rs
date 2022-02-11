// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::sync::Arc;

use ronvoy_core::event_loop::{self, EventLoop};
use ronvoy_proxy::{config::bootstrap, Ronvoy};

#[macro_export]
macro_rules! die(
    ($($arg:tt)*) => { {
        const EXIT_FAILURE: i32 = 1;
        eprintln!($($arg)*);
        std::process::exit(EXIT_FAILURE)
    } }
);

/// usage prints the help text about how to use ronvoy
fn usage() -> ! {
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "<ronvoy>".to_string());
    die!(
        concat!(
            "ronvoy {}: Envoy-compatible edge and service proxy.\n\
         \n\
         USAGE:\n",
            "    {} [OPTION...]\n",
            "\n\
         OPTIONS:\n",
            "    -h, --help          show this message\n",
            "    --config-path PATH  path to envoy bootstrap config JSON\n",
            "    --thread-pool       use thread-pool-based event loop (DEFAULT)\n",
            "    --independent       use multiple single-threaded event loops like Envoy\n",
        ),
        ronvoy_proxy::build_info::PKG_VERSION,
        argv0
    );
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Args {
    pub config_path: std::path::PathBuf,
    pub event_loop_kind: EventLoop,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut parsed = pico_args::Arguments::from_env();
    if parsed.contains(["-h", "--help"]) {
        usage();
    }

    let mut args = Args {
        config_path: "bootstrap.yaml".to_owned().into(),
        event_loop_kind: EventLoop::ThreadPool,
    };
    if let Ok(config_path) = parsed.value_from_str("--config-path") {
        args.config_path = config_path;
    }

    if parsed.contains("--independent") {
        if parsed.contains("--thread-pool") {
            eprintln!("ERROR: --thread-pool and --independent are mutually exclusive arguments");
            usage();
        }
        args.event_loop_kind = EventLoop::MultiSingleThreaded;
    } else if parsed.contains("--thread-pool") {
        args.event_loop_kind = EventLoop::ThreadPool;
    }

    Ok(args)
}

fn main() {
    let args = parse_args().unwrap();

    let num_threads: Option<usize> = {
        std::env::var("CONCURRENCY")
            .ok()
            .and_then(|s| s.parse().ok())
    };

    let bootstrap = bootstrap::load_config_sync(&args.config_path).unwrap();
    let ronvoy = Arc::new(Ronvoy::new(bootstrap).unwrap());

    println!("using {:?} event loop", args.event_loop_kind);
    event_loop::Builder::new(args.event_loop_kind)
        .worker_threads(num_threads)
        .build_and_block_on(|| {
            let ronvoy = ronvoy.clone();
            async move { ronvoy.start().await }
        })
        .unwrap();
}
