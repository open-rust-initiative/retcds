use rustls::{SupportedCipherSuite, ALL_CIPHER_SUITES};
use slog::info;
use crate::pkg::tlsutil::default_logger;

fn get_cipher_suite(s: &str) -> Option<u16> {


    for suite in ALL_CIPHER_SUITES.iter() {
        // info!(default_logger(),"{}",suite.suite().as_str().unwrap());
        if s == suite.suite().as_str().unwrap() {
            // info!(default_logger(),"success");
            return Some(suite.suite().get_u16());
        }
    }

    match s {
        "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305" => Some(0xcca8),
        "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305" => Some(0xcca9),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cipher_suite() {
        // Test that a valid cipher suite is correctly identified and returned
        assert_eq!(get_cipher_suite("TLS_AES_256_GCM_SHA384"), None);

        // Test that an unsupported cipher suite is not found
        assert_eq!(get_cipher_suite("TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"), Some(0xc02c));

        // Test that a custom cipher suite is correctly identified and returned
        assert_eq!(get_cipher_suite("TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305"), Some(0xcca8));

        // Test that another custom cipher suite is correctly identified and returned
        assert_eq!(get_cipher_suite("TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305"), Some(0xcca9));

        assert_eq!(get_cipher_suite("TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"),Some(0xc02f))
    }
}