// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::error::Error;
use std::net::{SocketAddr, TcpListener};

use socket2::SockAddr;

pub struct TcpListenerCloner {
    pub addr: SocketAddr,
    listener: Option<TcpListener>,
}

impl TcpListenerCloner {
    pub fn new(addr: SocketAddr) -> TcpListenerCloner {
        TcpListenerCloner {
            addr,
            listener: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn clone_listener(&mut self) -> Result<TcpListener, Box<dyn Error>> {
        if self.listener.is_none() {
            // SO_REUSEPORT on macOS is implemented such that _only_ the
            // last/most_recently created listener receives new connections.
            // This is different from the sane behavior of Linux (new requests are
            // load balanced across listeners in a SO_REUSEPORT group), so as a
            // workaround we create a single listener socket and `dup(2)` it
            // whenever clone_listener is called.  This will cause all dup'd sockets
            // to wake up and race to `accept(2)` a new connection, but its better
            // than the SO_REUSEPORT behavior.
            self.listener = Some(new_tcp_listener(self.addr, false)?);
        }

        let cloned = self.listener.as_ref().unwrap().try_clone()?;
        Ok(cloned)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn clone_listener(&mut self) -> Result<TcpListener, Box<dyn Error>> {
        // SO_REUSEPORT works great on linux, so create a new socket for each
        // clone_listener request.
        new_tcp_listener(addr, true)
    }
}

fn new_tcp_listener(addr: SocketAddr, reuse_port: bool) -> Result<TcpListener, Box<dyn Error>> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )
    .unwrap();

    socket.set_reuse_address(true).unwrap();
    if reuse_port {
        socket.set_reuse_port(true).unwrap();
    }

    socket.bind(&SockAddr::from(addr)).unwrap();
    socket.listen(128).unwrap();

    Ok(TcpListener::from(socket))
}
