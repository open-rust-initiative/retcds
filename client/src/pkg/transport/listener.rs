use std::ffi;
use std::fs::canonicalize;
use std::io::{Error, ErrorKind};
use std::ops::Shl;
use std::path::PathBuf;
use num_bigint::{BigUint, ToBigUint};
use openssl::asn1::{Asn1Integer, Asn1IntegerRef};
use openssl::bn::BigNum;
use openssl::nid::Nid;
use openssl::x509::{X509Builder, X509NameBuilder};
use openssl_sys::{BIGNUM, X509};
use rand::{Rng, thread_rng};
use rustls::{Certificate, Connection};
use slog::{Logger, warn};
use crate::pkg::fileutil::fileutil::torch_dir_all;
use crate::pkg::tlsutil::default_logger;

#[derive(Clone)]
struct TLSInfo {
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
    handshake_failure: Option<fn(&Connection, Error)>,

    // CipherSuites is a list of supported cipher suites.
    // If empty, Go auto-populates it by default.
    // Note that cipher suites are prioritized in the given order.
    cipher_suites: Vec<u16>,

    self_cert: bool,

    // parseFunc exists to simplify testing. Typically, parseFunc
    // should be left nil. In that case, tls.X509KeyPair will be used.
    parse_func: Option<fn(Vec<u8>, Vec<u8>) -> Result<Certificate, Error>>,

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
            cipher_suites: Vec::with_capacity(0),
            self_cert: false,
            parse_func: None,
            allowed_cn: String::from(""),
            allowed_hostname: String::from(""),
            logger: default_logger(),
            empty_cn: false,
        }
    }
    pub fn string(&self) -> String{
        format!("cert_file: {}, key_file: {}, client_cert_file: {}, client_key_file: {}, trusted_ca_file: {}, client_cert_auth: {}, CRL_file: {}, insecure_skip_verify: {}, skip_clientSAN_verify: {}, server_name: {}, allowed_cn: {}, allowed_hostname: {}, empty_cn: {}", self.cert_file, self.key_file, self.client_cert_file, self.client_key_file, self.trusted_ca_file, self.client_cert_auth, self.CRL_file, self.insecure_skip_verify, self.skip_clientSAN_verify, self.server_name, self.allowed_cn, self.allowed_hostname, self.empty_cn)
    }
    pub fn empty(&self) -> bool{
        return self.cert_file == "" && self.key_file == ""
    }
    pub fn self_cert(&mut self, dirpath:&str, hosts:Vec<String>, self_signed_cert_validity:usize, additional_usages:Vec<openssl::x509::extension::ExtendedKeyUsage>) -> Result<TLSInfo,Error>{
        self.logger = default_logger();
        if self_signed_cert_validity == 0{
            warn!(self.logger, "selfSignedCertValidity is invalid,it should be greater than 0,cannot generate cert");
            return Err(Error::new(ErrorKind::Other, "cannot generate cert"));
        };
        torch_dir_all(dirpath).expect("torch dir error");
        let mut cert_file = PathBuf::new().join(dirpath).join("cert.pem");
        let abs_cert_file = canonicalize(cert_file.clone()).expect("canonicalize error");
        let mut key_file = PathBuf::new().join(dirpath).join("key.pem");
        let abs_key_file = canonicalize(key_file.clone()).expect("canonicalize error");

        if abs_key_file.exists() && abs_cert_file.exists(){
            self.cert_file = abs_cert_file.clone().to_str().unwrap().to_string();
            self.key_file = abs_key_file.clone().to_str().unwrap().to_string();
            self.client_cert_file = abs_cert_file.clone().to_str().unwrap().to_string();
            self.client_key_file = abs_key_file.clone().to_str().unwrap().to_string();
            self.self_cert = true;
            return Ok(self.clone());
        }


        let mut rng = thread_rng();
        let serial_number_limit: BigUint = BigUint::from(1_u128).shl(128);
        let serial_number: BigUint = BigUint::from_bytes_le(&rng.gen::<[u8; 16]>()) % &serial_number_limit;
        if serial_number == BigUint::from(0_u128){
            warn!(self.logger, "serial number is 0, cannot generate cert");
            return Err(Error::new(ErrorKind::Other, "cannot generate cert"));
        }


        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_nid(Nid::ORGANIZATIONNAME,"etcd").unwrap();
        let num = BigNum::from_slice(serial_number.to_bytes_le().as_slice()).unwrap();
        let mut build = X509Builder::new().unwrap();
        build.set_serial_number(Asn1Integer::from_bn(&num).unwrap().as_ref()).unwrap();
        build.set_subject_name(name.build().as_ref()).unwrap();
        return Ok(self.clone());




    }
}

#[cfg(test)]
mod tests{
    use openssl::bn::BigNumContext;
    use openssl::rand::rand_bytes;
    use super::*;

    #[test]
    fn test(){
        let mut rng = thread_rng();
        let serial_number_limit: BigUint = BigUint::from(1_u128).shl(128);
        let serial_number: BigUint = BigUint::from_bytes_le(&rng.gen::<[u8; 16]>()) % &serial_number_limit;
        let num = BigNum::from_slice(serial_number.to_bytes_le().as_slice()).unwrap();
        let x = Asn1Integer::from_bn(&num).unwrap();

        println!("{}", x.to_bn().unwrap().to_string());
    }
}
