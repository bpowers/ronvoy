// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::collections::HashMap;
use std::future;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::Uri;
use envoy_control_plane::envoy::config::core::v3::{
    node::UserAgentVersionType, BuildVersion, Node,
};
use envoy_control_plane::envoy::r#type::v3::SemanticVersion;
use pico_args::Arguments;
use uuid::Uuid;

use crate::cluster::Cluster;
use crate::config::bootstrap;
use crate::extensions::filter::network::http_connection_manager::HttpConnectionManager;
use crate::listener::MakeHttpConnectionRouter;

#[cfg(test)]
use crate::testing::{TestHttpServer, TEST_HANDLER_RESPONSE};

#[cfg(test)]
mod testing;

mod address;
mod cluster;
pub mod config;
mod extensions;
mod listener;
mod route;
mod util;

pub(crate) mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub type Request = axum::http::Request<axum::body::Body>;
pub type Response = axum::http::Response<axum::body::Body>;

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
            "ronvoy {}: Edge and Service Proxy.\n\
         \n\
         USAGE:\n",
            "    {} [OPTION...]\n",
            "\n\
         OPTIONS:\n",
            "    -h, --help          show this message\n",
            "    --config-path PATH  path to envoy bootstrap config JSON\n",
        ),
        build_info::PKG_VERSION,
        argv0
    );
}

#[derive(PartialEq, Eq, Clone, Debug)]
struct Args {
    config_path: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut parsed = Arguments::from_env();
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

// the root datastructure for a Ronvoy instance
#[allow(dead_code)]
struct GlobalRonvoy {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        usage();
    });

    let bootstrap = bootstrap::load_config(&args.config_path).await?;

    let _num_threads = {
        std::env::var("CONCURRENCY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4)
    };

    let bootstrap = Arc::new(bootstrap);

    let mut node = bootstrap.node.clone().unwrap_or_else(|| Node {
        id: format!("ronvoy-{}", Uuid::new_v4()),
        ..Default::default()
    });

    // regardless of whats specified in the bootstrap config, ensure we specify that we are Ronvoy.
    node.user_agent_name = "ronvoy".to_string();
    node.user_agent_version_type =
        Some(UserAgentVersionType::UserAgentBuildVersion(BuildVersion {
            version: Some(SemanticVersion {
                major_number: build_info::PKG_VERSION_MAJOR.parse().unwrap_or_default(),
                minor_number: build_info::PKG_VERSION_MINOR.parse().unwrap_or_default(),
                patch: build_info::PKG_VERSION_PATCH.parse().unwrap_or_default(),
            }),
            metadata: None,
        }));

    #[allow(unused_variables)]
    let node = Arc::new(node);

    // let _cert = tokio::fs::read(&args.cert).await?;
    // let _key = tokio::fs::read(&args.key).await?;

    if let Some(static_resources) = bootstrap.static_resources.as_ref() {
        // build our upstream clusters
        let clusters: HashMap<String, Arc<cluster::Cluster>> = static_resources
            .clusters
            .iter()
            .filter_map(|cluster| {
                Cluster::try_from(cluster.clone())
                    .map(|cluster| (cluster.name.clone(), Arc::new(cluster)))
                    .ok()
            })
            .collect();

        eprintln!("clusters: {:?}", clusters);

        let clusters = Arc::new(arc_swap::ArcSwap::from_pointee(clusters));

        for listener in static_resources
            .listeners
            .iter()
            .cloned()
            .filter_map(|cfg| MakeHttpConnectionRouter::try_from((cfg, clusters.clone())).ok())
        {
            let addr = listener.listen_addr;
            println!("reverse proxy listening on {}", addr);
            let _ = tokio::spawn(axum::Server::bind(&addr).serve(listener));
        }
    }

    // TODO: this would be a good place for our xDS poller/receiver to live.
    //       for now wait forever (we spawned the listeners we needed above)
    future::pending::<()>().await;
    Ok(())
}

#[tokio::test]
async fn end_to_end_ronvoy() {
    let upstream = TestHttpServer::new();
    let upstream_url = format!("http://{}/", upstream.addr);

    // make sure the upstream is working as expected
    {
        let response = reqwest::get(&upstream_url).await.unwrap();
        assert!(response.status().is_success());
        let body = response.text().await.unwrap();
        assert_eq!(TEST_HANDLER_RESPONSE, body);
    }

    // start a gRPC server serving LDS and CDS

    // create a bootstrap config pointing the ADS to the gRPC server addr

    // create a new ronvoy instance off that bootstrap config
}
