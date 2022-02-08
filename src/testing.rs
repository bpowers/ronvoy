// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::net::SocketAddr;
use std::pin::Pin;

use axum::{routing::get, Router};
use envoy_control_plane::envoy::service::discovery::v3::{
    DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse,
};
use envoy_control_plane::envoy::service::listener::v3::listener_discovery_service_server::ListenerDiscoveryService;
use futures::Stream;
use tonic::{Request, Response, Status, Streaming};

pub(crate) const TEST_HANDLER_RESPONSE: &str = "hi there";

#[cfg(test)]
async fn test_handler() -> &'static str {
    TEST_HANDLER_RESPONSE
}

/// TestHttpServer starts an axum HTTP server listening on the `TestHttpServer.addr` address.
/// The server will be automatically shut down when TestHttpServer is dropped.
#[cfg(test)]
pub(crate) struct TestHttpServer {
    pub(crate) addr: SocketAddr,
    // when TestServer is Dropped, this field will be dropped, unblocking the graceful
    // await.  No need to implement a custom Drop impl, it will all work implicitly.
    #[allow(dead_code)]
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl TestHttpServer {
    pub(crate) fn new() -> Self {
        let app = Router::new().route("/", get(test_handler));

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let any_addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let server = axum::Server::bind(&any_addr).serve(app.into_make_service());

        let addr = server.local_addr();

        let server = server.with_graceful_shutdown(async {
            shutdown_rx.await.ok();
        });

        tokio::spawn(async move {
            server.await.unwrap();
        });

        Self { addr, shutdown_tx }
    }
}

#[derive(Default)]
pub struct StaticLDS {}

type DeltaStream = Pin<Box<dyn Stream<Item = Result<DeltaDiscoveryResponse, Status>> + Send>>;
type DiscoveryStream = Pin<Box<dyn Stream<Item = Result<DiscoveryResponse, Status>> + Send>>;

#[tonic::async_trait]
impl ListenerDiscoveryService for StaticLDS {
    type DeltaListenersStream = DeltaStream;
    async fn delta_listeners(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaListenersStream>, Status> {
        Err(Status::unimplemented(
            "delta listeners stream supported yet".to_owned(),
        ))
    }
    type StreamListenersStream = DiscoveryStream;
    async fn stream_listeners(
        &self,
        _request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamListenersStream>, Status> {
        Err(Status::unimplemented("TODO 2".to_owned()))
    }
    async fn fetch_listeners(
        &self,
        _request: Request<DiscoveryRequest>,
    ) -> Result<Response<DiscoveryResponse>, Status> {
        Err(Status::unimplemented("TODO 3".to_owned()))
    }
}
