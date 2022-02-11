// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

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
        ),
        ronvoy_proxy::build_info::PKG_VERSION,
        argv0
    );
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Args {
    pub config_path: std::path::PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut parsed = pico_args::Arguments::from_env();
    if parsed.contains(["-h", "--help"]) {
        usage();
    }

    let mut args = Args {
        config_path: "bootstrap.yaml".to_owned().into(),
    };
    if let Ok(config_path) = parsed.value_from_str("--config-path") {
        args.config_path = config_path;
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

    let mut rt = tokio::runtime::Builder::new_multi_thread();
    rt.enable_all();

    // if we've explicitly been told how many threads to use, only use that many
    if let Some(num_threads) = num_threads {
        rt.worker_threads(num_threads);
    }

    rt.build()
        .unwrap()
        .block_on(async {
            let bootstrap = bootstrap::load_config(&args.config_path).await?;

            let ronvoy = Ronvoy::new(bootstrap)?;
            ronvoy.start().await
        })
        .unwrap();
}
