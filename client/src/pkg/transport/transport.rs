use std::sync::Arc;
use std::time::Duration;
use hyper::Client;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use rustls::Error;
use tokio::net::unix::SocketAddr;
use crate::pkg::transport::listener::TLSInfo;

pub fn transport(tlsinfo:TLSInfo) -> Client<HttpsConnector<HttpConnector>, hyper::Body> {
    let config = tlsinfo.client_config();

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(config.as_ref().clone())
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();

    let client= Client::builder()
        .http2_keep_alive_timeout(Duration::from_secs(30))
        .build(https);
    return client;
}

// pub fn new_listener(addr: SocketAddr)

#[cfg(test)]
mod tests {
    use hyper::{Body, Response, Server};
    use hyper::server::conn::AddrIncoming;
    use hyper::service::{make_service_fn, service_fn};
    use hyper_rustls::TlsAcceptor;
    use openssl::x509::extension::ExtendedKeyUsage;
    use slog::warn;
    use tokio::fs::File;
    use tokio::io;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use super::*;
    use crate::pkg::transport::listener::{load_certs, load_private_key, new_tls_acceptor, TLSInfo};
    use crate::pkg::transport::listener::self_cert;
    use crate::pkg::tlsutil::default_logger;

    #[tokio::test]
    async fn test_transport() {
        let hosts = vec!["127.0.0.1"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        server(info.clone()).await;
        warn!(default_logger(),"client started");
        let client = transport(info);
        let response = client.get("https://127.0.0.1:5002".parse().unwrap()).await.unwrap();
        assert!(response.status().is_success());

        // println!("Response: {}", response.status());
    }

    async fn start_server(tlsinfo:TLSInfo) {
        let tls_acceptor = new_tls_acceptor(tlsinfo);
        let listener = TcpListener::bind("127.0.0.1:5002").await.unwrap();

        tokio::spawn(async move {
            let (stream, _peer_addr) = listener.accept().await.unwrap();
            println!("{}", _peer_addr.to_string());
            let mut tls_stream = tls_acceptor.accept(stream).await.unwrap();
            println!("server: Accepted client conn with TLS");

            let mut buf = [0; 22];
            tls_stream.read(&mut buf).await.unwrap();
            println!("server: got data: {:?}", buf);
            tls_stream.write(&buf).await.unwrap();
            println!("server: flush the data out");
        });
    }

    // #[tokio::test]
    async fn server(tlsinfo:TLSInfo){
        // let mut rt = tokio::runtime::Runtime::new().unwrap();
        let addr = "127.0.0.1:5002".parse().unwrap();

        let incoming = AddrIncoming::bind(&addr).unwrap();
        let tls_acceptor = new_tls_acceptor(tlsinfo.clone());

        let acceptor = TlsAcceptor::builder()
            .with_tls_config((*tlsinfo.clone().server_config()).clone())
            .with_all_versions_alpn()
            .with_incoming(incoming);
        let service = make_service_fn(|_| async { Ok::<_, io::Error>(service_fn(|_req|async {Ok::<_, io::Error>(Response::new(Body::empty()))})) });

        tokio::spawn(async move {
            let server = Server::builder(acceptor)
                .http2_only(true)
                .serve(service);
            server.await.unwrap();
        });
    }


}