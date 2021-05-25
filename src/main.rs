use std::io;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::net::ToSocketAddrs;
use std::thread;
use std::io::BufReader;
use async_executor::Executor;
use async_io::block_on;
use async_rustls::{ TlsConnector, rustls::ClientConfig, webpki::DNSNameRef };
use futures_lite::future;

#[derive(PartialEq, Eq, Clone, Default, Debug)]
struct Options {
    host: String,
    port: u16,
    domain: Option<String>,
    ca_file: Option<PathBuf>,
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
    let options = Options{
        host: "127.0.0.1".to_string(),
        port: 1337,
        domain: None,
        ca_file: None
    };

    let addr = (options.host.as_str(), options.port)
        .to_socket_addrs().unwrap()
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound)).unwrap();
    let domain = options.domain.unwrap_or(options.host);
    let content = format!(
        "GET / HTTP/1.0\r\nHost: {}\r\n\r\n",
        domain
    );

    let global_ronvoy = GlobalRonvoy {
        handshake_executor: Arc::new(Executor::new()),
        proxy_executor:  Arc::new(Executor::new()),
    };

    let num_threads = {
        // Parse SMOL_THREADS or default to 1.
        std::env::var("RONVOY_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4)
    };

    for n in 1..=num_threads {
        let handshake_executor = global_ronvoy.handshake_executor.clone();
        let proxy_executor = global_ronvoy.proxy_executor.clone();
        thread::Builder::new()
            .name(format!("handshake-{}", n))
            .spawn(move || {
                let handshake = &handshake_executor;
                loop {
                    block_on(handshake.run(future::pending::<()>()));
                }
            })
            .expect("cannot spawn executor thread");
    }


    // let mut runtime = runtime::Builder::new()
    //     .basic_scheduler()
    //     .enable_io()
    //     .build()?;
    // let mut config = ClientConfig::new();
    // if let Some(cafile) = &options.cafile {
    //     let mut pem = BufReader::new(File::open(cafile)?);
    //     config.root_store.add_pem_file(&mut pem)
    //         .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid cert"))?;
    // } else {
    //     config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    // }
    // let connector = TlsConnector::from(Arc::new(config));
    //
    // let fut = async {
    //     let stream = TcpStream::connect(&addr).await?;
    //
    //     let (mut stdin, mut stdout) = (tokio_stdin(), tokio_stdout());
    //
    //     let domain = DNSNameRef::try_from_ascii_str(&domain)
    //         .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;
    //
    //     let mut stream = connector.connect(domain, stream).await?;
    //     stream.write_all(content.as_bytes()).await?;
    //
    //     let (mut reader, mut writer) = split(stream);
    //     future::select(
    //         copy(&mut reader, &mut stdout),
    //         copy(&mut stdin, &mut writer)
    //     )
    //         .await
    //         .factor_first()
    //         .0?;
    //
    //     Ok(())
    // };
    //
    // runtime.block_on(fut)
}
