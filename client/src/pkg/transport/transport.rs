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
        .https_only()
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
    use openssl::x509::extension::ExtendedKeyUsage;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use super::*;
    use crate::pkg::transport::listener::{new_tls_acceptor, TLSInfo};
    use crate::pkg::transport::listener::self_cert;

    #[tokio::test]
    async fn test_transport() {
        let hosts = vec!["127.0.0.1"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        start_server(info.clone()).await;
        let client = transport(info);
        let response = client.get("https://127.0.0.1:5002".parse().unwrap()).await.unwrap();
        println!("Response: {}", response.status());
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
}