use crate::proxy::shadowsocks::context::SharedBloomContext;
use crate::proxy::{Address, BoxProxyStream, ChainableStreamBuilder};
use anyhow::anyhow;
use anyhow::Result;
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::io;
use crate::common::openssl_bytes_to_key;
use crate::proxy::shadowsocks::aead_helper::CipherKind;
use crate::proxy::shadowsocks::crypto_io::CryptoStream;

mod aead;
pub mod aead_helper;
pub mod context;
pub mod crypto_io;

fn ss_hkdf_sha1(iv_or_salt: &[u8], key: &[u8]) -> [u8; 64] {
    use hkdf::Hkdf;
    use sha1::Sha1;
    let ikm = key;
    let mut okm = [0u8; 64];
    let hk = Hkdf::<Sha1>::new(Some(iv_or_salt), ikm);
    hk.expand(b"ss-subkey", &mut okm)
        .expect("ss hkdf sha1 failed");
    okm
}

#[derive(Clone)]
pub struct ShadowsocksBuilder {
    addr: Address,
    method: CipherKind,
    context: SharedBloomContext,
    key: Bytes,
}

impl ShadowsocksBuilder {
    pub fn new(
        addr: Address,
        password: &str,
        method: &str,
        context: SharedBloomContext,
    ) -> Result<ShadowsocksBuilder> {
        let method = match method {
            "none" => CipherKind::None,
            "aes-128-gcm" => CipherKind::Aes128Gcm,
            "aes-256-gcm" => CipherKind::Aes256Gcm,
            "chacha20-ietf-poly1305" => CipherKind::ChaCha20Poly1305,
            _ => return Err(anyhow!("wrong ss encryption method")),
        };
        let mut key = BytesMut::with_capacity(method.key_len());
        unsafe {
            key.set_len(key.capacity());
        }
        openssl_bytes_to_key(password.as_bytes(), key.as_mut());
        Ok(ShadowsocksBuilder {
            addr,
            method,
            context,
            key: key.freeze(),
        })
    }
}

#[async_trait]
impl ChainableStreamBuilder for ShadowsocksBuilder {
    async fn build_tcp(&self, io: BoxProxyStream) -> io::Result<BoxProxyStream> {
        let mut stream = Box::new(CryptoStream::new(
            self.context.clone(),
            io,
            self.key.clone(),
            self.method,
        ));
        let res = self.addr.write_to_stream(&mut stream).await;
        match res {
            Ok(_) => Ok(stream),
            Err(e) => Err(e),
        }
    }

    async fn build_udp(&self, io: BoxProxyStream) -> io::Result<BoxProxyStream> {
        todo!()
    }

    fn into_box(self) -> Box<dyn ChainableStreamBuilder> {
        Box::new(self)
    }

    fn clone_box(&self) -> Box<dyn ChainableStreamBuilder> {
        Box::new(self.clone())
    }
}