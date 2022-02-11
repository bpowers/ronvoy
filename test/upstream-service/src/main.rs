// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

use std::error::Error;
use std::net::SocketAddr;

use axum::{routing::get, Router};
use ronvoy_core::event_loop;
use ronvoy_core::event_loop::EventLoop;
use ronvoy_core::net::TcpListenerCloner;

#[macro_export]
macro_rules! die(
    ($($arg:tt)*) => { {
        const EXIT_FAILURE: i32 = 1;
        eprintln!($($arg)*);
        std::process::exit(EXIT_FAILURE)
    } }
);

fn usage() -> ! {
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "<ronvoy>".to_string());
    die!(
        concat!(
            "upstream-service: simple HTTP server.\n\
         \n\
         USAGE:\n",
            "    {} [OPTION...]\n",
            "\n\
         OPTIONS:\n",
            "    -h, --help          show this message\n",
            "    --thread-pool       use thread-pool-based event loop (DEFAULT)\n",
            "    --independent       use multiple single-threaded event loops like Envoy\n",
        ),
        argv0
    );
}

fn parse_args() -> Result<EventLoop, Box<dyn Error>> {
    let mut parsed = pico_args::Arguments::from_env();
    if parsed.contains(["-h", "--help"]) {
        usage();
    }

    if parsed.contains("--independent") {
        if parsed.contains("--thread-pool") {
            eprintln!("ERROR: --thread-pool and --independent are mutually exclusive arguments");
            usage();
        }
        Ok(EventLoop::MultiSingleThreaded)
    } else {
        Ok(EventLoop::ThreadPool)
    }
}

fn main() {
    let event_loop_kind = parse_args().unwrap();

    let thread_count: Option<usize> = {
        std::env::var("CONCURRENCY")
            .ok()
            .and_then(|s| s.parse().ok())
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], 9110));
    println!("using {:?} event loop", event_loop_kind);

    let mut listener = TcpListenerCloner::new(addr);

    event_loop::Builder::new(event_loop_kind)
        .worker_threads(thread_count)
        .build_and_block_on(|| {
            let listener = listener.clone_listener().unwrap();
            eprintln!("listening on {}", listener.local_addr().unwrap());
            async {
                // build our application with a route
                let app = Router::new().route("/", get(handler));

                axum::Server::from_tcp(listener)
                    .unwrap()
                    .serve(app.into_make_service())
                    .await
                    .unwrap();

                Ok(())
            }
        })
        .unwrap();
}

async fn handler() -> &'static str {
    r#"{ "msg": "hithere" }"#
}
