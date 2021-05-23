use std::io;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::net::ToSocketAddrs;
use std::io::BufReader;
use async_executor::Executor;
use async_rustls::{ TlsConnector, rustls::ClientConfig, webpki::DNSNameRef };

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
    handshake_executor: Executor<'a>,
    // once a connection is established it goes here
    established_executor: Executor<'a>,

}

fn main() -> io::Result<()> {
    let options = Options::from_args();

    let addr = (options.host.as_str(), options.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let domain = options.domain.unwrap_or(options.host);
    let content = format!(
        "GET / HTTP/1.0\r\nHost: {}\r\n\r\n",
        domain
    );

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
