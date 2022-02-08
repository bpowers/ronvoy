// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::task::Poll;

use arc_swap::ArcSwapAny;
use axum::http::Uri;
use envoy_control_plane::envoy::config::cluster::v3::{
    cluster::LbPolicy as V3LbPolicy, Cluster as V3Cluster,
};
use envoy_control_plane::envoy::config::endpoint::v3::lb_endpoint::HostIdentifier;
use envoy_control_plane::envoy::config::endpoint::v3::Endpoint;

use crate::address::{self, Address};
use crate::util::response;

type Client = hyper::client::Client<hyper::client::HttpConnector>;

#[derive(Clone, Debug)]
pub enum LbPolicy {
    RoundRobin,
}

impl Default for LbPolicy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

impl TryFrom<V3LbPolicy> for LbPolicy {
    type Error = Infallible;

    fn try_from(value: V3LbPolicy) -> Result<Self, Self::Error> {
        let result = match value {
            V3LbPolicy::RoundRobin => LbPolicy::RoundRobin,
            V3LbPolicy::LeastRequest
            | V3LbPolicy::RingHash
            | V3LbPolicy::Random
            | V3LbPolicy::Maglev
            | V3LbPolicy::ClusterProvided
            | V3LbPolicy::LoadBalancingPolicyConfig => {
                eprintln!(
                    "TODO: unsupported V3LbPolicy {}, defaulting to RoundRobin",
                    value as i32
                );
                LbPolicy::RoundRobin
            }
        };
        Ok(result)
    }
}

/// Clusters is the updatable set of clusters a Ronvoy instance can route to
pub type Clusters = ArcSwapAny<Arc<HashMap<String, Arc<Cluster>>>>;

/// Cluster proxies requests to a specific set of upstream service instances
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct Cluster {
    pub name: String,
    client: Client,
    lb_policy: LbPolicy,
    endpoints: Arc<Vec<Address>>,
    off: Arc<AtomicUsize>, // used to index endpoints for round robin LB policy
}

impl tower::Service<axum::http::Request<axum::body::Body>> for Cluster {
    type Response = axum::http::Response<axum::body::Body>;
    type Error = Infallible;

    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _ctx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: circuit breaker/rate limit here
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: axum::http::Request<axum::body::Body>) -> Self::Future {
        let off = self.off.clone();
        let endpoints = self.endpoints.clone();
        let client = self.client.clone();
        Box::pin(async move {
            println!("Handling a request for {}", req.uri());
            let off = off.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let Address::Socket(endpoint) = &endpoints[off % endpoints.len()];

            let path = req.uri().path();
            let path_query = req
                .uri()
                .path_and_query()
                .map(|v| v.as_str())
                .unwrap_or(path);

            let uri = format!("http://{}{}", endpoint, path_query);

            *req.uri_mut() = Uri::try_from(uri).unwrap();

            match client.request(req).await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    let msg = format!("upstream error: {}", err);
                    Ok(response::json_error(503, &msg))
                }
            }
        })
    }
}

impl TryFrom<V3Cluster> for Cluster {
    type Error = Box<dyn Error>;

    fn try_from(v3_cluster: V3Cluster) -> Result<Self, Self::Error> {
        let lb_policy = LbPolicy::try_from(
            V3LbPolicy::from_i32(v3_cluster.lb_policy).unwrap_or(V3LbPolicy::RoundRobin),
        )?;

        let load_assignment = v3_cluster.load_assignment.unwrap_or_default();
        let endpoints = Arc::new(
            load_assignment
                .endpoints
                .into_iter()
                .flat_map(|locality_endpoints| {
                    locality_endpoints
                        .lb_endpoints
                        .into_iter()
                        .filter_map(|endpoint| {
                            if let Some(HostIdentifier::Endpoint(Endpoint {
                                address: Some(address),
                                ..
                            })) = endpoint.host_identifier
                            {
                                address::Address::try_from(address).ok()
                            } else {
                                None
                            }
                        })
                })
                .collect(),
        );

        // TODO: transport socket with TLS client cert

        Ok(Cluster {
            name: v3_cluster.name,
            client: Default::default(),
            lb_policy,
            endpoints,
            off: Arc::new(Default::default()),
        })
    }
}
