// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::net::SocketAddr;
use std::collections::HashMap;

use axum::{extract::Path, response::Json, routing::get, Router};

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new().route("/:echo", get(handler));

    // run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 9110));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(Path(params): Path<HashMap<String, String>>) -> Json<serde_json::Value> {
    use serde_json::json;

    Json(json!({ "path": params.get("echo") }))
}
