// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

// use mimalloc::MiMalloc;
//
// #[global_allocator]
// static GLOBAL: MiMalloc = MiMalloc;

use std::fs::File;
use std::io::{self, BufReader};
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use async_executor::Executor;
use async_io::block_on;
use async_net::TcpListener;
use async_rustls::rustls::internal::msgs::enums::ProtocolVersion;
use async_rustls::rustls::internal::pemfile::{certs, pkcs8_private_keys};
use async_rustls::rustls::{Certificate, NoClientAuth, PrivateKey, ServerConfig};
use async_rustls::TlsAcceptor;
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
            "    --cert FILE      server certificate to use\n",
            "    --key FILE       server private key to use\n",
        ),
        VERSION,
        argv0
    );
}

#[derive(PartialEq, Eq, Clone, Debug)]
struct Args {
    addr: String,
    port: u16,
    cert: PathBuf,
    key: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut parsed = Arguments::from_env();
    if parsed.contains(["-h", "--help"]) {
        usage();
    }

    let mut args = Args {
        addr: "127.0.0.1".to_owned(),
        port: 9301,
        cert: "".to_owned().into(),
        key: "".to_owned().into(),
    };
    if let Ok(addr) = parsed.value_from_str("--addr") {
        args.addr = addr;
    }
    if let Ok(port) = parsed.value_from_str::<&str, String>("--port") {
        args.port = port.parse::<u16>().unwrap();
    }
    args.cert = parsed.value_from_str("--cert").unwrap();
    args.key = parsed.value_from_str("--key").unwrap();

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

fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
    certs(&mut BufReader::new(File::open(path)?))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid cert"))
}

fn load_keys(path: &Path) -> io::Result<Vec<PrivateKey>> {
    pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid key"))
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

    let certs = load_certs(&args.cert).unwrap();
    let mut keys = load_keys(&args.key).unwrap();
    let mut config = ServerConfig::new(NoClientAuth::new());
    config
        .set_single_cert(certs, keys.remove(0))
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
        .unwrap();
    config.versions = vec![ProtocolVersion::TLSv1_3];
    config.set_protocols(&["h2".as_bytes().to_vec(), "http/1.1".as_bytes().to_vec()]);

    let addr = (args.addr.as_str(), args.port)
        .to_socket_addrs()
        .unwrap()
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        .unwrap();

    let acceptor = TlsAcceptor::from(Arc::new(config));
    let handshake_executor = global_ronvoy.handshake_executor.clone();
    let proxy_executor = global_ronvoy.proxy_executor.clone();

    let fut = async move {
        let listener = TcpListener::bind(addr).await.unwrap();

        loop {
            let (stream, peer_addr) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let proxy_executor = (&proxy_executor).clone();

            eprintln!(
                "accepted new connection from {} ({:?})",
                peer_addr,
                thread::current().id()
            );
            let fut = async move {
                let mut stream = acceptor.accept(stream).await?;
                let fut = async move {
                    eprintln!(
                        "we accepted, what now? closing. ({:?})",
                        thread::current().id()
                    );
                    stream.close().await.unwrap();
                };

                // move the rest of computation onto the proxy thread pool
                // (alternatively, pick one of $n independent executors to
                // move it to rather than a global pool, like envoy, in the
                // future)
                let proxy_executor = (&proxy_executor).clone();
                proxy_executor.spawn(fut).detach();

                Ok::<(), Box<dyn std::error::Error>>(())
            };

            handshake_executor
                .spawn(async move {
                    if let Err(err) = fut.await {
                        eprintln!("listener err: {} ({:?})", err, thread::current().id())
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
