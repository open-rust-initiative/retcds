use std::fs::read;
use std::path::Path;
use openssl::error::ErrorStack;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use openssl::stack::Stack;
use openssl::x509::{X509, X509Name};


pub fn new_name_list(ca_files: &[String]) -> Result<Stack<X509Name>, ErrorStack> {

    let mut res = Stack::new().unwrap();
    for ca_file in ca_files {
        let name = X509Name::load_client_ca_file(ca_file).unwrap();
        for na in name {
            let push = res.push(na);
            if push.is_err() {
                return Err(ErrorStack::get());
            }
        }
    }
    return Ok(res);
}

pub fn new_cert_default(certfile: &str, keyfile: &str) -> Result<SslAcceptor, ErrorStack> {
    let cert = read(Path::new(certfile)).unwrap();
    let key = read(Path::new(keyfile)).unwrap();
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
    builder.set_private_key_file(keyfile, SslFiletype::PEM)?;
    builder.set_certificate_chain_file(certfile)?;
    // builder.build();

    Ok(builder.build())

}



pub fn new_cert(certfile: &str, keyfile: &str, parse_func: Option<impl FnMut(&[u8], &[u8]) -> Result<SslAcceptor, ErrorStack>>) -> Result<SslAcceptor, ErrorStack>{
    let cert = read(Path::new(certfile)).unwrap();
    let key = read(Path::new(keyfile)).unwrap();

    let acceptor = if let Some(mut parse_func) = parse_func {
        parse_func(&cert, &key)?
    } else {
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
        builder.set_private_key_file(keyfile, SslFiletype::PEM)?;
        builder.set_certificate_chain_file(certfile)?;
        builder.build()
    };

    Ok(acceptor)

}

#[cfg(test)]
mod tests{
    use std::io::Write;
    use std::path::PathBuf;
    use slog::info;
    use tempfile::NamedTempFile;
    use crate::pkg::tlsutil::default_logger;
    use crate::pkg::tlsutil::tlsutil::{new_cert, new_cert_default, new_name_list};

    #[test]
    fn test_new_cert_pool(){
        let ca_files = vec!["invalid_path1".to_string(), "invalid_path2".to_string()];

        let result = new_name_list(&ca_files);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_cert_with_default_parser() {
        let certfile = "test_cert.crt";
        let keyfile = "test_key.key";
        let result = new_cert_default(certfile, keyfile);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_cert_with_default_parse_func() {
        // 创建临时证书和密钥文件。
        let cert_contents = b"test cert contents";
        let key_contents = b"test key contents";
        let mut cert_file = NamedTempFile::new().unwrap();
        let mut key_file = NamedTempFile::new().unwrap();
        let cert_path = PathBuf::from("src/pkg/tlsutil/cert.pem");
        let key_path = PathBuf::from("src/pkg/tlsutil/key.pem");
        cert_file.write_all(cert_contents).unwrap();
        key_file.write_all(key_contents).unwrap();

        // 调用被测试函数。
        let acceptor = new_cert_default(cert_path.to_str().unwrap(), key_path.to_str().unwrap());

        // info!(default_logger(),"{}",acceptor.unwrap().context().session_cache_size())

        // 断言结果。
        assert!(acceptor.is_ok());
    }
}