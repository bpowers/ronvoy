// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::net::{SocketAddr, TcpListener};

use axum::{routing::get, Router};
use socket2::SockAddr;

fn main() {
    let num_threads = {
        std::env::var("CONCURRENCY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4)
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], 9110));
    println!("listening on {}", addr);

    let mut children = vec![];
    for _i in 0..num_threads {
        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .unwrap();
        socket.set_reuse_address(true).unwrap();
        socket.set_reuse_port(true).unwrap();
        socket.bind(&SockAddr::from(addr)).unwrap();
        socket.listen(128).unwrap();
        let listener = TcpListener::from(socket);

        children.push(std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let listener = listener;
                    // build our application with a route
                    let app = Router::new().route("/", get(handler));

                    axum::Server::from_tcp(listener)
                        .unwrap()
                        .serve(app.into_make_service())
                        .await
                        .unwrap();
                });
        }));
    }

    for child in children {
        let _ = child.join();
    }
}

async fn handler() -> &'static str {
    r#"{ "msg": "hithere" }"#
}
