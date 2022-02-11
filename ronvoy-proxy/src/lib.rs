// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::sync::Arc;

use envoy_control_plane::envoy::config::bootstrap::v3::Bootstrap;
use envoy_control_plane::envoy::config::core::v3::Node;

mod address;
pub mod build_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
mod cluster;
pub mod config;
mod extensions;
mod listener;
mod route;
#[cfg(test)]
mod testing;
mod util;

pub type Request = axum::http::Request<axum::body::Body>;
pub type Response = axum::http::Response<axum::body::Body>;

/// Ronvoy is the root datastructure for a Ronvoy instance
/// TODO: should we not have Arcs here, and instead Arc the Ronvoy instance?
pub struct Ronvoy {
    pub bootstrap_config: Arc<Bootstrap>,
    pub node: Arc<Node>,
    pub clusters: Arc<cluster::Clusters>,
}

impl Ronvoy {
    /// new creates a new Ronvoy instance from a given bootstrap config.
    pub fn new(bootstrap_config: Bootstrap) -> Result<Ronvoy, Box<dyn std::error::Error>> {
        let clusters = get_bootstrap_clusters(&bootstrap_config)?;
        let node = get_node(bootstrap_config.node.as_ref());

        Ok(Ronvoy {
            bootstrap_config: Arc::new(bootstrap_config),
            node: Arc::new(node),
            clusters: Arc::new(clusters),
        })
    }

    /// start creates listeners and gets Ronvoy to begin accepting requests.
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bootstrap = self.bootstrap_config.clone();

        let listeners: Vec<_> = if let Some(static_resources) = bootstrap.static_resources.as_ref()
        {
            static_resources
                .listeners
                .iter()
                .cloned()
                .filter_map(|cfg| {
                    listener::MakeHttpConnectionRouter::try_from((cfg, self.clusters.clone())).ok()
                })
                .collect()
        } else {
            vec![]
        };

        for listener in listeners.into_iter() {
            let addr = listener.listen_addr;
            println!("ronvoy listening on {}", addr);
            let _ = tokio::spawn(axum::Server::bind(&addr).serve(listener));
        }

        // TODO: xDS connection and updates
        std::future::pending::<()>().await;

        Ok(())
    }
}

/// get_bootstrap_clusters creates Clusters that can proxy HTTP requests from an Envoy bootstrap configuration
pub fn get_bootstrap_clusters(
    bootstrap_config: &Bootstrap,
) -> Result<cluster::Clusters, Box<dyn std::error::Error>> {
    let clusters: std::collections::HashMap<String, Arc<cluster::Cluster>> =
        if let Some(static_resources) = bootstrap_config.static_resources.as_ref() {
            // build our upstream clusters
            static_resources
                .clusters
                .iter()
                .filter_map(|cluster| {
                    cluster::Cluster::try_from(cluster.clone())
                        .map(|cluster| (cluster.name.clone(), Arc::new(cluster)))
                        .ok()
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

    let clusters = arc_swap::ArcSwap::from_pointee(clusters);

    Ok(clusters)
}

/// get_node returns or creates an Envoy v3 config Node object with "ronvoy" (and our version) as the user agent.
fn get_node(bootstrap_node: Option<&Node>) -> Node {
    use envoy_control_plane::envoy::config::core::v3::{node::UserAgentVersionType, BuildVersion};
    use envoy_control_plane::envoy::r#type::v3::SemanticVersion;

    let mut node = bootstrap_node.cloned().unwrap_or_else(|| Node {
        id: format!("ronvoy-{}", uuid::Uuid::new_v4()),
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

    node
}

#[tokio::test]
async fn end_to_end_ronvoy() {
    use crate::testing::{TestHttpServer, TEST_HANDLER_RESPONSE};

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
