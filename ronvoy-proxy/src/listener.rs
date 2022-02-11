// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use anyhow::{anyhow, Error as AnyhowError};
use envoy_control_plane::envoy::config::listener::v3::filter::ConfigType as V3ConfigType;
use envoy_control_plane::envoy::config::listener::v3::Listener as V3Listener;
use envoy_control_plane::envoy::extensions::filters::network::http_connection_manager::v3::HttpConnectionManager as V3HttpConnectionManager;
use hyper::server::conn::AddrStream;
use hyper::service::Service;
use ronvoy_core::response;

use crate::cluster::Clusters;
use crate::extensions::filter::network::http_connection_manager::HttpConnectionManager;

/// MakeHttpConnectionRouter is called when a new TCP connection is opened to us from a downstream client.
#[derive(Clone, Debug)]
pub struct MakeHttpConnectionRouter {
    pub listen_addr: SocketAddr,
    http_conn_mgr: Arc<HttpConnectionManager>,
}

impl MakeHttpConnectionRouter {
    pub fn new(http_conn_mgr: HttpConnectionManager, addr: SocketAddr) -> Self {
        Self {
            listen_addr: addr,
            http_conn_mgr: Arc::new(http_conn_mgr),
        }
    }
}

impl<'t> Service<&'t AddrStream> for MakeHttpConnectionRouter {
    type Error = Infallible;
    type Response = HttpConnectionRouter;
    type Future = Pin<Box<dyn Future<Output = Result<HttpConnectionRouter, Infallible>> + Send>>;

    fn poll_ready(&mut self, _ctx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: circuit breaker/rate limit here
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, target: &'t AddrStream) -> Self::Future {
        let remote_addr = target.remote_addr();
        let listen_addr = self.listen_addr;
        let http_conn_mgr = self.http_conn_mgr.clone();
        Box::pin(async move {
            Ok(HttpConnectionRouter {
                listen_addr,
                remote_addr,
                http_conn_mgr,
            })
        })
    }
}

/// HttpConnectionRouter handles HTTP Requests that come in over a single connection
#[derive(Clone, Debug)]
pub struct HttpConnectionRouter {
    #[allow(dead_code)]
    listen_addr: SocketAddr,
    #[allow(dead_code)]
    remote_addr: SocketAddr,
    http_conn_mgr: Arc<HttpConnectionManager>,
}

impl tower::Service<axum::http::Request<axum::body::Body>> for HttpConnectionRouter {
    type Response = axum::http::Response<axum::body::Body>;
    type Error = Infallible;

    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _ctx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: circuit breaker/rate limit here
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: axum::http::Request<axum::body::Body>) -> Self::Future {
        let cluster = self.http_conn_mgr.get_cluster(&req);
        Box::pin(async move {
            if let Some(cluster) = cluster {
                // the routing layer found a cluster we should send the request to
                let mut c = (&*cluster).clone();
                c.call(req).await
            } else {
                Ok(response::json_error(
                    404,
                    "routing to upstream cluster failed",
                ))
            }

            // TODO: log line
        })
    }
}

impl TryFrom<(V3Listener, Arc<Clusters>)> for MakeHttpConnectionRouter {
    type Error = AnyhowError;

    fn try_from((listener, clusters): (V3Listener, Arc<Clusters>)) -> Result<Self, Self::Error> {
        let filter_chain = &listener.filter_chains[0];
        let filter = &filter_chain.filters[0];
        if filter.name != "envoy.filters.network.http_connection_manager" {
            return Err(anyhow!(
                "expected 'envoy.filters.network.http_connection_manager' filter, not {}",
                filter.name
            ));
        }

        use envoy_control_plane::prost::Message;
        use envoy_control_plane::prost_wkt_types::MessageSerde;

        let http_conn_mgr_type_url = V3HttpConnectionManager::default().type_url();

        let v3_http_conn_mgr = if let Some(V3ConfigType::TypedConfig(http_conn_mgr_any)) =
            filter.config_type.as_ref()
        {
            if http_conn_mgr_any.type_url == http_conn_mgr_type_url {
                V3HttpConnectionManager::decode(&*http_conn_mgr_any.value)?
            } else {
                return Err(anyhow!(
                    "unsupported typed config: {}",
                    &http_conn_mgr_any.type_url
                ));
            }
        } else {
            return Err(anyhow!("expected TypedConfig"));
        };

        let http_conn_mgr = HttpConnectionManager::try_from((v3_http_conn_mgr, clusters))?;

        // TODO: transport socket

        if let Some(addr) = listener.address.clone() {
            let crate::address::Address::Socket(addr) = crate::address::Address::try_from(addr)?;
            Ok(MakeHttpConnectionRouter::new(http_conn_mgr, addr))
        } else {
            Err(anyhow!("expected listener to specify address"))
        }
    }
}
