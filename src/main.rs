// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::io;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use async_executor::Executor;
use async_io::block_on;
use async_net::TcpListener;
use async_rustls::{rustls::NoClientAuth, rustls::ServerConfig, TlsAcceptor};
use futures_lite::{future, AsyncWriteExt};
use pico_args::Arguments;

const VERSION: &str = "0.1";

#[macro_export]
macro_rules! die(
    ($($arg:tt)*) => { {
        use std;
        const EXIT_FAILURE: i32 = 1;
        eprintln!($($arg)*);
        std::process::exit(EXIT_FAILURE)
    } }
);

fn usage() -> ! {
    let argv0 = std::env::args()
        .next()
        .unwrap_or_else(|| "<ronvoy>".to_string());
    die!(
        concat!(
            "ronvoy {}: Edge and Service Proxy.\n\
         \n\
         USAGE:\n",
            "    {} [OPTION...]\n",
            "\n\
         OPTIONS:\n",
            "    -h, --help       show this message\n",
            "    --addr           address to listen on (defaults to 127.0.0.1)\n",
            "    --port           port to listen on (defaults to 9301)\n",
            "    --ca_file FILE   Certificate Authority file (defaults to webpki_roots)\n",
        ),
        VERSION,
        argv0
    );
}

#[derive(PartialEq, Eq, Clone, Debug)]
struct Args {
    addr: String,
    port: u16,
    ca_file: Option<PathBuf>,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut parsed = Arguments::from_env();
    if parsed.contains(["-h", "--help"]) {
        usage();
    }

    let mut args = Args {
        addr: "127.0.0.1".to_string(),
        port: 9301,
        ca_file: None,
    };
    if let Ok(addr) = parsed.value_from_str("--addr") {
        args.addr = addr;
    }
    if let Ok(port) = parsed.value_from_str::<&str, String>("port") {
        args.port = port.parse::<u16>().unwrap();
    }
    args.ca_file = parsed.value_from_str("--ca_file").ok();

    Ok(args)
}

// the root datastructure for a Ronvoy instance
struct GlobalRonvoy<'a> {
    // TLS handshakes are expensive - segregate them from proxying
    // data back and forth on established connections
    handshake_executor: Arc<Executor<'a>>,
    // once a connection is established it goes here
    proxy_executor: Arc<Executor<'a>>,
}

fn main() {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        usage();
    });

    let global_ronvoy = GlobalRonvoy {
        handshake_executor: Arc::new(Executor::new()),
        proxy_executor: Arc::new(Executor::new()),
    };

    let num_threads = {
        std::env::var("RONVOY_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4)
    };

    for n in 1..=num_threads {
        let handshake_executor = global_ronvoy.handshake_executor.clone();
        thread::Builder::new()
            .name(format!("handshake-{}", n))
            .spawn(move || {
                let handshake = &handshake_executor;
                eprintln!(
                    "started handshake-{} thread {:?}",
                    n,
                    thread::current().id()
                );
                loop {
                    block_on(handshake.run(future::pending::<()>()));
                }
            })
            .expect("cannot spawn executor thread");
    }

    for n in 1..=num_threads {
        let proxy_executor = global_ronvoy.proxy_executor.clone();
        thread::Builder::new()
            .name(format!("proxy-{}", n))
            .spawn(move || {
                let proxy = &proxy_executor;
                eprintln!("started proxy-{} thread {:?}", n, thread::current().id());
                loop {
                    block_on(proxy.run(future::pending::<()>()));
                }
            })
            .expect("cannot spawn executor thread");
    }

    let config = ServerConfig::new(NoClientAuth::new());

    let addr = (args.addr.as_str(), args.port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        .unwrap();

    let acceptor = TlsAcceptor::from(Arc::new(config));
    let handshake_executor = global_ronvoy.handshake_executor.clone();
    let _proxy_executor = global_ronvoy.proxy_executor.clone();

    let fut = async move {
        let listener = TcpListener::bind(addr).await.unwrap();

        loop {
            let (stream, peer_addr) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();

            eprintln!("accepted new connection from {}", peer_addr);
            let fut = async move {
                let mut stream = acceptor.accept(stream).await?;

                eprintln!("we accepted, what now?  need to move it to the next executor");

                stream.close().await.unwrap();

                Ok::<(), Box<dyn std::error::Error>>(())
            };

            handshake_executor
                .spawn(async move {
                    if let Err(err) = fut.await {
                        eprintln!("listener err: {}", err)
                    }
                })
                .detach();
        }
    };

    let listener_executor = Executor::new();
    eprintln!(
        "started listener on main thread {:?}",
        thread::current().id()
    );

    block_on(listener_executor.run(fut));

    eprintln!("listener exited, so ronvoy exiting");
}
