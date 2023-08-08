use std::fmt::Pointer;
use std::fs::{File, Permissions, set_permissions};
use std::io::{Error, ErrorKind, Write};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::ops::{Deref, Shl};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use num_bigint::{BigUint, ToBigUint};
use openssl::asn1::{Asn1Integer, Asn1Time};
use openssl::bn::BigNum;
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslConnector, SslConnectorBuilder, SslMethod, SslVerifyMode, SslVersion};
use openssl::x509::{X509Builder, X509NameBuilder};
use openssl::x509::extension::{ExtendedKeyUsage, KeyUsage, SubjectAlternativeName};
use rand::{Rng, thread_rng};
use rand::distributions::uniform::SampleBorrow;
use slog::{info, Logger, warn};
use crate::pkg::fileutil::fileutil::torch_dir_all;
use crate::pkg::tlsutil::default_logger;
use std::os::fd::AsRawFd;
use std::convert::TryFrom;
use std::io::BufReader;
use openssl::rsa::Rsa;
use rustls::{RootCertStore, server::{AllowAnyAuthenticatedClient}, SupportedCipherSuite};
use rustls::internal::msgs::codec::Codec;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{
    TlsAcceptor,
    TlsConnector,
    rustls::{self},
    client::TlsStream as ClientTlsStream,
};

#[derive(Clone,Debug)]
pub struct TLSInfo {
    // CertFile is the _server_ cert, it will also be used as a _client_ certificate if ClientCertFile is empty
    cert_file: String,
    // KeyFile is the key for the CertFile
    key_file: String,
    // ClientCertFile is a _client_ cert for initiating connections when ClientCertAuth is defined. If ClientCertAuth
    // is true but this value is empty, the CertFile will be used instead.
    client_cert_file: String,
    // ClientKeyFile is the key for the ClientCertFile
    client_key_file: String,

    trusted_ca_file: String,
    client_cert_auth: bool,
    CRL_file: String,
    insecure_skip_verify: bool,
    skip_clientSAN_verify: bool,

    // ServerName ensures the cert matches the given host in case of discovery / virtual hosting
    server_name: String,

    // HandshakeFailure is optionally called when a connection fails to handshake. The
    // connection will be closed immediately afterwards.
    handshake_failure: Option<fn(&[u8], &[u8]) -> Result<SslAcceptor, ErrorStack>>,

    // CipherSuites is a list of supported cipher suites.
    cipher_suites: Vec<SupportedCipherSuite>,

    self_cert: bool,

    // AllowedCN is a CN which must be provided by a client.
    allowed_cn: String,

    // AllowedHostname is an IP address or hostname that must match the TLS
    // certificate provided by a client.
    allowed_hostname: String,

    // Logger logs TLS errors.
    // If nil, all logs are discarded.
    logger: Logger,

    // EmptyCN indicates that the cert must have empty CN.
    // If true, ClientConfig() will return an error for a cert with non empty CN.
    empty_cn: bool,
}

impl TLSInfo{
    pub fn new() -> Self{
        TLSInfo{
            cert_file: String::from(""),
            key_file: String::from(""),
            client_cert_file: String::from(""),
            client_key_file: String::from(""),
            trusted_ca_file: String::from(""),
            client_cert_auth: false,
            CRL_file: String::from(""),
            insecure_skip_verify: false,
            skip_clientSAN_verify: false,
            server_name: String::from(""),
            handshake_failure: None,
            cipher_suites: rustls::ALL_CIPHER_SUITES.to_vec(),
            self_cert: false,
            allowed_cn: String::from(""),
            allowed_hostname: String::from(""),
            logger: default_logger(),
            empty_cn: false,
        }
    }
    pub fn string(&self) -> String{
       return  format!("cert_file: {}, key_file: {}, client_cert_file: {}, client_key_file: {}, trusted_ca_file: {}, client_cert_auth: {}, CRL_file: {}, insecure_skip_verify: {}, skip_clientSAN_verify: {}, server_name: {}, allowed_cn: {}, allowed_hostname: {}, empty_cn: {}, self_check: {}", self.cert_file, self.key_file, self.client_cert_file, self.client_key_file, self.trusted_ca_file, self.client_cert_auth, self.CRL_file, self.insecure_skip_verify, self.skip_clientSAN_verify, self.server_name, self.allowed_cn, self.allowed_hostname, self.empty_cn,self.self_cert);
    }
    pub fn empty(&self) -> bool{
        return self.cert_file == "" && self.key_file == ""
    }

    pub fn get_key_file(&self) -> String{
        return self.key_file.clone();
    }

    pub fn get_cert_file(&self) -> String{
        return self.cert_file.clone();
    }

    // cafiles returns a list of CA file paths.
    pub fn cafiles(&self) -> Result<Vec<String>,Error>{
        let mut cafiles = vec![];
        if self.trusted_ca_file != ""{
            cafiles.push(self.trusted_ca_file.clone());
        }
        return Ok(cafiles);
    }

    // ServerConfig generates a config use by an HTTP server.
    pub fn server_config(&self) -> Arc<rustls::ServerConfig> {
        let roots = load_certs(self.cert_file.as_str());
        // warn!(default_logger(),"{}",self.cert_file.as_str());
        let certs = roots.clone();

        let certfile = File::open(self.cert_file.as_str()).expect("cannot open certificate file");
        let mut reader = BufReader::new(certfile);
        let mut client_auth_roots = RootCertStore::empty();
        // for root in roots {
        //     client_auth_roots.add_parsable_certificates(rustls_pemfile::certs(& reader)).unwrap();
        // }

        client_auth_roots.add_parsable_certificates(&rustls_pemfile::certs(&mut reader).unwrap());
        let client_auth = AllowAnyAuthenticatedClient::new(client_auth_roots);
        // warn!(default_logger(),"{}",self.key_file.as_str());
        let privkey = load_private_key(self.key_file.as_str());
        // let suites = rustls::ALL_CIPHER_SUITES.to_vec();
        let versions = rustls::ALL_VERSIONS.to_vec();

        let mut config = rustls::ServerConfig::builder()
            .with_cipher_suites(&self.cipher_suites)
            .with_safe_default_kx_groups()
            .with_protocol_versions(&versions)
            .expect("inconsistent cipher-suites/versions specified")
            .with_client_cert_verifier(client_auth.boxed())
            .with_single_cert(certs, privkey)
            // .with_single_cert_with_ocsp_and_sct(certs, privkey, vec![], vec![])
            .expect("bad certificates/private key");


        config.key_log = Arc::new(rustls::KeyLogFile::new());
        config.session_storage = rustls::server::ServerSessionMemoryCache::new(256);
        Arc::new(config)
    }


    pub fn client_config(&self) -> Arc<rustls::ClientConfig> {

        let mut root_store = RootCertStore::empty();
        if self.trusted_ca_file != ""{
            let cert_file = File::open(self.trusted_ca_file.as_str()).expect("Cannot open CA file");
            let mut reader = BufReader::new(cert_file);
            root_store.add_parsable_certificates(&rustls_pemfile::certs(&mut reader).unwrap());
        }
        else {
            let cert_file = File::open(self.client_cert_file.as_str()).expect("Cannot open CA file");
            let mut reader = BufReader::new(cert_file);
            root_store.add_parsable_certificates(&rustls_pemfile::certs(&mut reader).unwrap());
        }



        // let suites = rustls::DEFAULT_CIPHER_SUITES.to_vec();
        let versions = rustls::DEFAULT_VERSIONS.to_vec();

        let certs = load_certs(self.client_cert_file.as_str());
        let key = load_private_key(self.client_key_file.as_str());


        let config = rustls::ClientConfig::builder()
            .with_cipher_suites(&self.cipher_suites)
            .with_safe_default_kx_groups()
            .with_protocol_versions(&versions)
            .expect("inconsistent cipher-suite/versions selected")
            .with_root_certificates(root_store)
            // .with_root_certificates(root_store)
            .with_client_auth_cert(certs, key)
            .expect("invalid client auth certs/key");
        Arc::new(config)
    }

    pub async fn new_tls_stream(&self,domain: &str, addr: SocketAddr) -> ClientTlsStream<TcpStream> {
        let config = self.client_config();

        let connector = TlsConnector::from(config);

        let stream = TcpStream::connect(&addr).await.unwrap();
        let domain = rustls::ServerName::try_from(domain)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname")).unwrap();
        let stream = connector.connect(domain, stream).await.unwrap();
        stream
    }

    pub fn new_tls_acceptor(&self) -> TlsAcceptor {
        let config = self.server_config();
        let acceptor = TlsAcceptor::from(config);
        acceptor
    }
}

pub fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
    let certfile = File::open(filename).expect("cannot open certificate file");
    let mut reader = BufReader::new(certfile);

    rustls_pemfile::certs(&mut reader)
        .unwrap()
        .iter()
        .map(|v| rustls::Certificate(v.clone()))
        .collect()
}

pub fn load_private_key(filename: &str) -> rustls::PrivateKey {
    let keyfile = File::open(filename).expect("cannot open private key file");
    let mut reader = BufReader::new(keyfile);
    loop {
        match rustls_pemfile::read_one(&mut reader).expect("cannot parse private key .pem file") {
            Some(rustls_pemfile::Item::RSAKey(key)) => {
                // warn!(default_logger(),"is RSAkey");
                return rustls::PrivateKey(key)},
            Some(rustls_pemfile::Item::PKCS8Key(key)) => {
                // warn!(default_logger(),"is pkcs8key");
                return rustls::PrivateKey(key)},
            Some(rustls_pemfile::Item::ECKey(key)) => {
                // warn!(default_logger(),"is ECkey");
                return rustls::PrivateKey(key)},
            None => break,
            _ => {}
        }
    }

    panic!(
        "no keys found in {:?} (encrypted keys not supported)",
        filename
    );
}

pub fn lookup_ipv4(host: &str, port: u16) -> SocketAddr {
    let addrs = (host, port).to_socket_addrs().unwrap();
    for addr in addrs {
        if let SocketAddr::V4(_) = addr {
            return addr;
        }
    }

    unreachable!("Cannot lookup address");
}

pub fn self_cert(dirpath:&str, hosts:Vec<&str>, self_signed_cert_validity:usize, additional_usages:Option<&mut ExtendedKeyUsage>) -> Result<TLSInfo,Error>{
    let mut info = TLSInfo::new();
    info.logger = default_logger();
    if self_signed_cert_validity == 0{
        warn!(info.logger, "selfSignedCertValidity is invalid,it should be greater than 0,cannot generate cert");
        return Err(Error::new(ErrorKind::Other, "cannot generate cert"));
    };
    torch_dir_all(dirpath).expect("torch dir error");
    let mut cert_file = PathBuf::new().join(dirpath).join("cert.pem");
    let mut abs_cert_file = cert_file.clone();
    let mut key_file = PathBuf::new().join(dirpath).join("key.pem");
    let mut abs_key_file = key_file.clone();
    // abs_key_file.push("key.pem");

    if abs_key_file.exists() && abs_cert_file.exists(){
        info.cert_file = abs_cert_file.clone().to_str().unwrap().to_string();
        info.key_file = abs_key_file.clone().to_str().unwrap().to_string();
        info.client_cert_file = abs_cert_file.clone().to_str().unwrap().to_string();
        info.client_key_file = abs_key_file.clone().to_str().unwrap().to_string();
        info.self_cert = true;
        return Ok(info.clone());
    }


    let mut rng = thread_rng();
    let serial_number_limit: BigUint = BigUint::from(1_u128).shl(128);
    let serial_number: BigUint = BigUint::from_bytes_le(&rng.gen::<[u8; 16]>()) % &serial_number_limit;
    if serial_number == BigUint::from(0_u128){
        warn!(info.logger, "serial number is 0, cannot generate cert");
        return Err(Error::new(ErrorKind::Other, "cannot generate cert"));
    }


    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_nid(Nid::ORGANIZATIONNAME,"etcd").unwrap();
    let key_usage = KeyUsage::new().key_encipherment().digital_signature().build().unwrap();
    let ext_key_usage = ExtendedKeyUsage::new().server_auth().build().unwrap();
    let num = BigNum::from_slice(serial_number.to_bytes_le().as_slice()).unwrap();
    let mut build = X509Builder::new().unwrap();

    build.set_serial_number(Asn1Integer::from_bn(&num).unwrap().as_ref()).unwrap();
    build.set_subject_name(name.build().as_ref()).unwrap();
    build.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
    let after =&Asn1Time::days_from_now(self_signed_cert_validity as u32).unwrap();
    build.set_not_after(&after.clone()).unwrap();
    build.append_extension(key_usage).unwrap();
    if additional_usages.is_some(){
        build.append_extension(additional_usages.unwrap().server_auth().build().unwrap()).unwrap()
    }
    else {
        build.append_extension(ext_key_usage).unwrap();
    };
    info!(default_logger(),"automatically generate certificates,certificate-validity-bound-not-after =>{}",after.clone().to_string());

    let mut ext_builder = SubjectAlternativeName::new();
    for host in hosts.clone() {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            ext_builder.ip(&ip.to_string());
        } else {
            ext_builder.dns(host);
        }
    }
    let ext = ext_builder.build(&build.x509v3_context(None, None)).unwrap();
    build.append_extension(ext).unwrap();
    build.set_version(2).unwrap();
    let mut issuer = X509NameBuilder::new().unwrap();
    issuer.append_entry_by_nid(Nid::ORGANIZATIONNAME,"etcd").unwrap();
    build.set_issuer_name(issuer.build().as_ref()).unwrap();

    // let group = EcGroup::from_curve_name(Nid::SECP521R1).unwrap();
    // let ecdsa = EcKey::generate(&group);
    let rsa = Rsa::generate(2048);
    if rsa.clone().is_err(){
        warn!(default_logger(),"cannot generate rsa key, reason =>{}",rsa.clone().err().unwrap());
        return Err(Error::new(ErrorKind::Other, "cannot generate ecdsa key"));
    }
    let priv_key = PKey::from_rsa(rsa.clone().unwrap()).unwrap();

    build.set_pubkey(PKey::public_key_from_pem(priv_key.clone().public_key_to_pem().unwrap().as_slice()).unwrap().as_ref()).unwrap();
    build.sign(priv_key.clone().as_ref(), MessageDigest::sha256()).unwrap();

    let mut cert = build.build();
    let pem = cert.to_pem();
    if pem.is_err(){
        warn!(default_logger(),"cannot generate pem, reason =>{}",pem.clone().err().unwrap());
        return Err(Error::new(ErrorKind::Other, "cannot generate pem"));
    }
    let cert_out = File::create(cert_file.clone()).unwrap()
        .write_all(cert.to_pem().unwrap().as_slice());


    if cert_out.is_err() {
        warn!(default_logger(),"cannot write cert file, reason =>{}",cert_out.err().unwrap().to_string().clone());
        return Err(Error::new(ErrorKind::Other, "cannot write cert file"));
    }

    let key_out = File::create(key_file.clone()).unwrap()
        .write_all(rsa.unwrap().private_key_to_pem().unwrap().as_slice());
    if key_out.is_err() {
        warn!(default_logger(),"cannot write key file, reason =>{}",key_out.err().unwrap().to_string().clone());
        return Err(Error::new(ErrorKind::Other, "cannot write key file"));
    }
    set_permissions(key_file.clone(),Permissions::from_mode(0o600)).unwrap();
    return self_cert(dirpath.clone(), hosts.clone(), self_signed_cert_validity.clone(), None);
}

pub fn new_tls_acceptor(tlsinfo:TLSInfo) -> TlsAcceptor {
    let config = tlsinfo.server_config();
    let acceptor = TlsAcceptor::from(config);
    acceptor
}

#[cfg(test)]
mod tests{
    use std::io::BufReader;
    use openssl::x509::extension::{ExtendedKeyUsage, KeyUsage};
    use openssl::x509::X509;
    use super::*;

    #[test]
    fn test(){
        let mut rng = thread_rng();
        let serial_number_limit: BigUint = BigUint::from(1_u128).shl(128);
        let serial_number: BigUint = BigUint::from_bytes_le(&rng.gen::<[u8; 16]>()) % &serial_number_limit;
        let num = BigNum::from_slice(serial_number.to_bytes_le().as_slice()).unwrap();
        let x = Asn1Integer::from_bn(&num).unwrap();
        let key_usage = KeyUsage::new().key_encipherment().digital_signature().build().unwrap();
        println!("{}", x.to_bn().unwrap().to_string());
    }

    #[test]
    fn test_self_cert(){
        let mut tls_info = TLSInfo::new();
        let hosts = vec!["127.0.0.1"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let tls_info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages));
        println!("{:?}", tls_info.unwrap().string());
    }

}
