// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::error::Error as StdError;
use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

const INITIAL_BUFFER_CAPACITY: usize = 32 * 1024;

/// read_all_utf8 reads a file from disk and returns it as a UTF8-valid string
pub(crate) async fn read_all_utf8(path: impl AsRef<Path>) -> Result<String, Box<dyn StdError>> {
    let mut file = File::open(path).await?;
    let mut buf = Vec::with_capacity(INITIAL_BUFFER_CAPACITY);
    file.read_to_end(&mut buf).await?;
    let contents = String::from_utf8(buf)?;
    Ok(contents)
}
