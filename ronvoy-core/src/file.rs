// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::error::Error as StdError;
use std::path::Path;

use tokio::fs::File;

const INITIAL_BUFFER_CAPACITY: usize = 32 * 1024;

/// read_all_utf8 reads a file from disk and returns it as a UTF8-valid string
pub async fn read_all_utf8(path: impl AsRef<Path>) -> Result<String, Box<dyn StdError>> {
    use tokio::io::AsyncReadExt;

    let mut file = File::open(path).await?;
    let mut buf = Vec::with_capacity(INITIAL_BUFFER_CAPACITY);
    file.read_to_end(&mut buf).await?;
    let contents = String::from_utf8(buf)?;
    Ok(contents)
}

/// read_all_utf8_sync reads a file from disk and returns it as a UTF8-valid string
pub fn read_all_utf8_sync(path: impl AsRef<Path>) -> Result<String, Box<dyn StdError>> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(INITIAL_BUFFER_CAPACITY);
    file.read_to_end(&mut buf)?;
    let contents = String::from_utf8(buf)?;
    Ok(contents)
}
