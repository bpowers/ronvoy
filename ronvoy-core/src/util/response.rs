// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use axum::http::StatusCode;

/// json_error returns a `Response` with a JSON body describing the error and the Content-Type header set.
pub(crate) fn json_error<T>(status: T, msg: &str) -> crate::Response
where
    StatusCode: TryFrom<T>,
    <StatusCode as TryFrom<T>>::Error: Into<axum::http::Error>,
{
    // FIXME: YOLO escaping is bad and I feel bad
    let json_body = format!("{{\"error\": \"{}\"}}", msg.replace("\"", "\\\""));
    axum::http::Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(json_body))
        .unwrap()
}
