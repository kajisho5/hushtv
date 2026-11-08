//! 6 桁コードでのペアリング（Android 11 以降の「ワイヤレスデバッグ」）。Google の adb は使わない。
//!
//! テレビの［ワイヤレスデバッグ］→［ペア設定コードによるデバイスのペア設定］で出る
//! 「IP アドレスとポート」と「6 桁のコード」で、このパソコンの鍵をテレビに覚えさせる。
//! 覚えさせたあとは、別のポート（接続用）に TLS でつなげば、許可の確認なしで使える。
//!
//! 手順（adbd の pairing_connection と同じ）：
//! 1. ペアリング用のポートに TLS 1.3 でつなぐ（どちらも相手の証明書は確かめない）
//! 2. パスワード ＝ コードの文字 ＋ TLS の鍵素材 64 バイト（ラベル `adb-label\0`）
//! 3. SPAKE2（BoringSSL の方式）を 1 往復
//! 4. 共有鍵から AES-128-GCM の鍵を作り、こちらの公開鍵を送って、テレビの GUID を受け取る
//!
//! SPAKE2 の部分は libadb（MIT、<https://github.com/tarigo/libadb>）の pairing/spake2.rs を元に、
//! BoringSSL の crypto/curve25519/spake25519.cc と突き合わせて書いた。

use rsa::BigUint;
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PairError {
    #[error("コードは 6 桁の数字です")]
    BadCode,
    #[error(
        "ペアリング用のポートにつながりません（{0}）。テレビの画面が開いたままか確かめてください"
    )]
    Connect(String),
    #[error("暗号化の準備に失敗しました（{0}）")]
    Tls(String),
    #[error("鍵を読めません（{0}）")]
    Key(String),
    #[error("コードが違います。テレビに出ている 6 桁を入れ直してください")]
    WrongCode,
    #[error("テレビとのやりとりが途中で切れました（{0}）")]
    Protocol(String),
}

const EXPORTER_LABEL: &[u8] = b"adb-label\0";
const EXPORTER_LEN: usize = 64;
const CLIENT_NAME: &[u8] = b"adb pair client\0";
const SERVER_NAME: &[u8] = b"adb pair server\0";
const HKDF_INFO: &[u8] = b"adb pairing_auth aes-128-gcm key";
const PEER_INFO_SIZE: usize = 8192;
const TAG_LEN: usize = 16;
const MAX_PAYLOAD: usize = 16384;

/// 6 桁のコードとして正しいか。
pub fn valid_code(code: &str) -> bool {
    code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit())
}

/// ペアリングする。成功するとテレビの GUID（接続用ポートの mDNS 名にもなる）を返す。
pub fn pair(addr: SocketAddr, code: &str, key_path: &Path) -> Result<String, PairError> {
    let code = code.trim();
    if !valid_code(code) {
        return Err(PairError::BadCode);
    }
    crate::ensure_key(key_path).map_err(|e| PairError::Key(e.to_string()))?;
    let pem = std::fs::read_to_string(key_path).map_err(|e| PairError::Key(e.to_string()))?;
    let public_line = public_key_line(&pem)?;

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| PairError::Connect(e.to_string()))?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(20)));
    let _ = tcp.set_nodelay(true);
    let config = tls_config(&pem)?;
    let name = rustls::pki_types::ServerName::IpAddress(addr.ip().into());
    let conn = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|e| PairError::Tls(e.to_string()))?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    // 1 回書いて読むまでの間に TLS の握手を済ませる
    while tls.conn.is_handshaking() {
        tls.conn
            .complete_io(&mut tls.sock)
            .map_err(|e| PairError::Tls(e.to_string()))?;
    }

    let mut exported = [0u8; EXPORTER_LEN];
    tls.conn
        .export_keying_material(&mut exported, EXPORTER_LABEL, None)
        .map_err(|e| PairError::Tls(e.to_string()))?;
    let mut password = code.as_bytes().to_vec();
    password.extend_from_slice(&exported);

    let mut wide = [0u8; 64];
    getrandom::fill(&mut wide).map_err(|e| PairError::Tls(e.to_string()))?;
    let spake = spake2::Spake2::new(
        spake2::Role::Alice,
        CLIENT_NAME,
        SERVER_NAME,
        &password,
        &wide,
    );
    wide.fill(0);
    password.fill(0);

    send(&mut tls, 0, spake.message())?;
    let theirs = recv(&mut tls, 0)?;
    let shared = spake
        .finish(&theirs)
        .ok_or_else(|| PairError::Protocol("テレビの返事が曲線上の点ではありません".into()))?;
    let mut cipher = cipher::Cipher::new(&shared);

    let mut info = vec![0u8; PEER_INFO_SIZE];
    info[0] = 0; // ADB_RSA_PUB_KEY
    let bytes = public_line.as_bytes();
    if bytes.len() > PEER_INFO_SIZE - 2 {
        return Err(PairError::Key("公開鍵が長すぎます".into()));
    }
    info[1..1 + bytes.len()].copy_from_slice(bytes);
    send(&mut tls, 1, &cipher.seal(&info))?;

    let answer = recv(&mut tls, 1)?;
    if answer.len() != PEER_INFO_SIZE + TAG_LEN {
        return Err(PairError::WrongCode);
    }
    let opened = cipher.open(&answer).ok_or(PairError::WrongCode)?;
    if opened[0] != 1 {
        return Err(PairError::Protocol(format!(
            "テレビの返事の種類が違います（{}）",
            opened[0]
        )));
    }
    let data = &opened[1..];
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    Ok(String::from_utf8_lossy(&data[..end]).into_owned())
}

fn send<S: Write>(s: &mut S, kind: u8, payload: &[u8]) -> Result<(), PairError> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.push(1); // バージョン
    out.push(kind);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    s.write_all(&out)
        .and_then(|_| s.flush())
        .map_err(|e| PairError::Protocol(e.to_string()))
}

fn recv<S: Read>(s: &mut S, kind: u8) -> Result<Vec<u8>, PairError> {
    let mut header = [0u8; 6];
    s.read_exact(&mut header).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            // テレビはコードが違うと黙って切ることがある
            PairError::WrongCode
        } else {
            PairError::Protocol(e.to_string())
        }
    })?;
    if header[0] != 1 {
        return Err(PairError::Protocol(format!("バージョン番号 {}", header[0])));
    }
    if header[1] != kind {
        return Err(PairError::Protocol(format!(
            "種類 {}（{} のはず）",
            header[1], kind
        )));
    }
    let len = u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize;
    if len == 0 || len > MAX_PAYLOAD {
        return Err(PairError::Protocol(format!("長さ {len}")));
    }
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload)
        .map_err(|e| PairError::Protocol(e.to_string()))?;
    Ok(payload)
}

/// `adbkey.pub` と同じ形（Android の公開鍵の並びを base64 にしたもの ＋ 空白 ＋ 名前）。
pub fn public_key_line(pem: &str) -> Result<String, PairError> {
    use base64::Engine;
    let key = rsa::RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| PairError::Key(e.to_string()))?;
    let bytes = android_pubkey(key.n(), key.e())?;
    let host = hostname();
    Ok(format!(
        "{} hushtv@{host}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// system/core/libcrypto_utils/android_pubkey.cpp の android_pubkey_encode と同じ並び。
fn android_pubkey(n: &BigUint, e: &BigUint) -> Result<Vec<u8>, PairError> {
    const WORDS: usize = 64; // 2048 ビット
    let bytes = WORDS * 4;
    if n.bits() > bytes * 8 {
        return Err(PairError::Key("2048 ビットの鍵だけ使えます".into()));
    }
    let r32 = BigUint::from(1u64 << 32);
    let n0 = low_u32(&(n % &r32));
    // n0inv = -1 / n0 mod 2^32（Newton 法）
    let mut inv: u32 = 1;
    for _ in 0..5 {
        inv = inv.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(inv)));
    }
    let n0inv = inv.wrapping_neg();
    let r = BigUint::from(1u8) << (bytes * 8);
    let rr = (&r * &r) % n;
    if e.bits() > 32 {
        return Err(PairError::Key("指数が読めません".into()));
    }
    let e = low_u32(e);
    let mut out = Vec::with_capacity(4 + 4 + bytes * 2 + 4);
    out.extend_from_slice(&(WORDS as u32).to_le_bytes());
    out.extend_from_slice(&n0inv.to_le_bytes());
    out.extend_from_slice(&le_padded(n, bytes));
    out.extend_from_slice(&le_padded(&rr, bytes));
    out.extend_from_slice(&e.to_le_bytes());
    Ok(out)
}

fn low_u32(v: &BigUint) -> u32 {
    let b = le_padded(v, 4);
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn le_padded(v: &BigUint, len: usize) -> Vec<u8> {
    let mut b = v.to_bytes_le();
    b.resize(len.max(b.len()), 0);
    b
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "computer".into())
}

fn tls_config(pem: &str) -> Result<rustls::ClientConfig, PairError> {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let kp = rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(pem, &rcgen::PKCS_RSA_SHA256)
        .map_err(|e| PairError::Key(e.to_string()))?;
    let cert = rcgen::CertificateParams::new(Vec::<String>::new())
        .and_then(|p| p.self_signed(&kp))
        .map_err(|e| PairError::Key(e.to_string()))?;
    let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der()));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| PairError::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCert(provider)))
        .with_client_auth_cert(vec![CertificateDer::from(cert.der().to_vec())], der)
        .map_err(|e| PairError::Tls(e.to_string()))
}

/// ペアリングのサーバー証明書はその場で作られたもので、確かめようがない（adb も確かめない）。
/// 相手が本物かは、6 桁のコードを知っているか（SPAKE2）で確かめる。
#[derive(Debug)]
struct AnyCert(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

mod cipher {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes128Gcm, Key, Nonce};

    /// 共有鍵から HKDF-SHA256 で AES-128-GCM の鍵を作る。nonce は送り・受けそれぞれの通し番号（下位 8 バイト、リトルエンディアン）。
    pub struct Cipher {
        aead: Aes128Gcm,
        enc: u64,
        dec: u64,
    }

    impl Cipher {
        pub fn new(shared: &[u8]) -> Self {
            let mut key = [0u8; 16];
            hkdf::Hkdf::<sha2::Sha256>::new(None, shared)
                .expand(super::HKDF_INFO, &mut key)
                .expect("16 バイトは HKDF の上限より短い");
            let aead = Aes128Gcm::new(&Key::<Aes128Gcm>::from(key));
            key.fill(0);
            Self {
                aead,
                enc: 0,
                dec: 0,
            }
        }

        fn nonce(n: u64) -> Nonce<aes_gcm::aes::cipher::consts::U12> {
            let mut b = [0u8; 12];
            b[..8].copy_from_slice(&n.to_le_bytes());
            Nonce::from(b)
        }

        pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
            let out = self
                .aead
                .encrypt(&Self::nonce(self.enc), plain)
                .expect("GCM はこの長さを暗号化できる");
            self.enc += 1;
            out
        }

        pub fn open(&mut self, sealed: &[u8]) -> Option<Vec<u8>> {
            let out = self.aead.decrypt(&Self::nonce(self.dec), sealed).ok()?;
            self.dec += 1;
            Some(out)
        }
    }
}

pub mod spake2 {
    //! BoringSSL の SPAKE2（edwards25519）。RFC 9382 とも crates.io の `spake2` とも違う。
    //! 要点：M と N は素数位数の部分群の外にある点なので、掛ける数は mod l で縮めずにそのまま使う。

    use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
    use curve25519_dalek::scalar::Scalar;
    use curve25519_dalek::traits::Identity;
    use sha2::{Digest, Sha512};
    use subtle::{Choice, ConditionallySelectable};

    /// SHA256("edwards25519 point generation seed (M)")。BoringSSL の表のコメントと同じ値。
    pub const M: [u8; 32] = [
        0x5a, 0xda, 0x7e, 0x4b, 0xf6, 0xdd, 0xd9, 0xad, 0xb6, 0x62, 0x6d, 0x32, 0x13, 0x1c, 0x6b,
        0x5c, 0x51, 0xa1, 0xe3, 0x47, 0xa3, 0x47, 0x8f, 0x53, 0xcf, 0xcf, 0x44, 0x1b, 0x88, 0xee,
        0xd1, 0x2e,
    ];
    /// SHA256("edwards25519 point generation seed (N)")。
    pub const N: [u8; 32] = [
        0x10, 0xe3, 0xdf, 0x0a, 0xe3, 0x7d, 0x8e, 0x7a, 0x99, 0xb5, 0xfe, 0x74, 0xb4, 0x46, 0x72,
        0x10, 0x3d, 0xbd, 0xdc, 0xbd, 0x06, 0xaf, 0x68, 0x0d, 0x71, 0x32, 0x9a, 0x11, 0x69, 0x3b,
        0xc7, 0x78,
    ];
    /// 部分群の位数 l（リトルエンディアン）。
    const ORDER: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde,
        0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
    ];

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Role {
        Alice,
        Bob,
    }

    pub struct Spake2 {
        role: Role,
        my_name: Vec<u8>,
        their_name: Vec<u8>,
        private_key: [u8; 32],
        password_hash: [u8; 64],
        password_scalar: [u8; 32],
        my_msg: [u8; 32],
    }

    fn add(a: &mut [u8; 32], b: &[u8; 32]) {
        let mut carry = 0u16;
        for i in 0..32 {
            let s = u16::from(a[i]) + u16::from(b[i]) + carry;
            a[i] = s as u8;
            carry = s >> 8;
        }
    }

    fn left_shift_3(n: &mut [u8; 32]) {
        let mut carry = 0u8;
        for b in n.iter_mut() {
            let next = *b >> 5;
            *b = (*b << 3) | carry;
            carry = next;
        }
    }

    /// 縮めない 256 ビットの数での掛け算（定数時間の倍加と加算）。
    fn mul_raw(p: &EdwardsPoint, k: &[u8; 32]) -> EdwardsPoint {
        let mut acc = EdwardsPoint::identity();
        for byte in k.iter().rev() {
            for bit in (0..8).rev() {
                acc = acc + acc;
                let sum = acc + p;
                acc = EdwardsPoint::conditional_select(&acc, &sum, Choice::from((byte >> bit) & 1));
            }
        }
        acc
    }

    fn point(b: &[u8; 32]) -> EdwardsPoint {
        CompressedEdwardsY(*b)
            .decompress()
            .expect("M と N は曲線上の点")
    }

    fn absorb(h: &mut Sha512, data: &[u8]) {
        h.update((data.len() as u64).to_le_bytes());
        h.update(data);
    }

    /// パスワードの数：SHA-512 を mod l で縮め、下 3 ビットが 0 になるまで l・2l・4l を足す（縮め直さない）。
    pub fn password_scalar(hash: &[u8; 64]) -> [u8; 32] {
        let mut s = Scalar::from_bytes_mod_order_wide(hash).to_bytes();
        let mut order = ORDER;
        for bit in 0..3 {
            let m = 1u8 << bit;
            let take = Choice::from((s[0] & m) >> bit);
            let mut addend = [0u8; 32];
            for (d, o) in addend.iter_mut().zip(order.iter()) {
                *d = u8::conditional_select(&0, o, take);
            }
            add(&mut s, &addend);
            let copy = order;
            add(&mut order, &copy);
        }
        debug_assert_eq!(s[0] & 7, 0);
        s
    }

    impl Spake2 {
        /// `random` は 64 バイトの乱数（自分の秘密の数になる）。
        pub fn new(
            role: Role,
            my_name: &[u8],
            their_name: &[u8],
            password: &[u8],
            random: &[u8; 64],
        ) -> Self {
            let mut private_key = Scalar::from_bytes_mod_order_wide(random).to_bytes();
            left_shift_3(&mut private_key);
            let password_hash: [u8; 64] = Sha512::digest(password).into();
            let password_scalar = password_scalar(&password_hash);
            let mask = point(if role == Role::Alice { &M } else { &N });
            // 基点は素数位数なので、ここは縮めても同じ結果になる
            let base = EdwardsPoint::mul_base(&Scalar::from_bytes_mod_order(private_key));
            let my_msg = (base + mul_raw(&mask, &password_scalar))
                .compress()
                .to_bytes();
            Self {
                role,
                my_name: my_name.to_vec(),
                their_name: their_name.to_vec(),
                private_key,
                password_hash,
                password_scalar,
                my_msg,
            }
        }

        pub fn message(&self) -> &[u8; 32] {
            &self.my_msg
        }

        /// 相手の 32 バイトから 64 バイトの共有鍵を作る（コードが合っていたかはまだ分からない）。
        pub fn finish(mut self, their: &[u8]) -> Option<[u8; 64]> {
            let their: [u8; 32] = their.try_into().ok()?;
            let q: EdwardsPoint = CompressedEdwardsY(their).decompress()?;
            let peer_mask = point(if self.role == Role::Alice { &N } else { &M });
            let unmasked = q - mul_raw(&peer_mask, &self.password_scalar);
            let shared = mul_raw(&unmasked, &self.private_key).compress().to_bytes();
            let (an, bn, am, bm) = match self.role {
                Role::Alice => (&self.my_name, &self.their_name, &self.my_msg, &their),
                Role::Bob => (&self.their_name, &self.my_name, &their, &self.my_msg),
            };
            let mut h = Sha512::new();
            absorb(&mut h, an);
            absorb(&mut h, bn);
            absorb(&mut h, am);
            absorb(&mut h, bm);
            absorb(&mut h, &shared);
            absorb(&mut h, &self.password_hash);
            self.private_key.fill(0);
            self.password_scalar.fill(0);
            self.password_hash.fill(0);
            Some(h.finalize().into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::spake2::*;
    use super::*;

    fn rnd(seed: u8) -> [u8; 64] {
        std::array::from_fn(|i| seed.wrapping_add(i as u8).wrapping_mul(31))
    }

    #[test]
    fn mask_points_are_the_hashed_seeds() {
        use sha2::{Digest, Sha256};
        let m: [u8; 32] = Sha256::digest(b"edwards25519 point generation seed (M)").into();
        let n: [u8; 32] = Sha256::digest(b"edwards25519 point generation seed (N)").into();
        assert_eq!(m, M);
        assert_eq!(n, N);
    }

    #[test]
    fn same_code_same_key() {
        let a = Spake2::new(Role::Alice, CLIENT_NAME, SERVER_NAME, b"123456xyz", &rnd(1));
        let b = Spake2::new(Role::Bob, SERVER_NAME, CLIENT_NAME, b"123456xyz", &rnd(2));
        let (am, bm) = (*a.message(), *b.message());
        let ka = a.finish(&bm).unwrap();
        let kb = b.finish(&am).unwrap();
        assert_eq!(ka, kb);
        // 暗号化して相手が開ける
        let mut ca = cipher::Cipher::new(&ka);
        let mut cb = cipher::Cipher::new(&kb);
        let sealed = ca.seal(b"hello");
        assert_eq!(cb.open(&sealed).unwrap(), b"hello");
    }

    #[test]
    fn wrong_code_different_key() {
        let a = Spake2::new(Role::Alice, CLIENT_NAME, SERVER_NAME, b"123456", &rnd(3));
        let b = Spake2::new(Role::Bob, SERVER_NAME, CLIENT_NAME, b"654321", &rnd(4));
        let (am, bm) = (*a.message(), *b.message());
        let mut ca = cipher::Cipher::new(&a.finish(&bm).unwrap());
        let mut cb = cipher::Cipher::new(&b.finish(&am).unwrap());
        assert!(cb.open(&ca.seal(b"hello")).is_none());
    }

    #[test]
    fn password_scalar_is_multiple_of_eight_and_unreduced() {
        for s in 0..40u8 {
            let h: [u8; 64] = std::array::from_fn(|i| s.wrapping_mul(7).wrapping_add(i as u8));
            let p = password_scalar(&h);
            assert_eq!(p[0] & 7, 0);
        }
    }

    #[test]
    fn code_check() {
        assert!(valid_code("123456"));
        assert!(!valid_code("12345"));
        assert!(!valid_code("12a456"));
    }

    #[test]
    fn pubkey_layout() {
        use rsa::pkcs8::{EncodePrivateKey, LineEnding};
        let key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
        let line = public_key_line(&pem).unwrap();
        let b64 = line.split(' ').next().unwrap();
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert_eq!(raw.len(), 4 + 4 + 256 + 256 + 4);
        assert_eq!(u32::from_le_bytes(raw[0..4].try_into().unwrap()), 64);
        let n0inv = u32::from_le_bytes(raw[4..8].try_into().unwrap());
        let n0 = u32::from_le_bytes(raw[8..12].try_into().unwrap());
        assert_eq!(n0.wrapping_mul(n0inv), u32::MAX); // n0 * n0inv ≡ -1 (mod 2^32)
        assert_eq!(u32::from_le_bytes(raw[520..524].try_into().unwrap()), 65537);
    }
}

/// テレビ側（adbd のペアリングサーバー）の真似。手元で一連の流れを確かめるためだけに使う。
#[cfg(test)]
mod fake_tv {
    use super::*;
    use std::net::TcpListener;

    #[derive(Debug)]
    struct AnyClient(Arc<rustls::crypto::CryptoProvider>);

    impl rustls::server::danger::ClientCertVerifier for AnyClient {
        fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
            &[]
        }
        fn verify_client_cert(
            &self,
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &[rustls::pki_types::CertificateDer<'_>],
            _: rustls::pki_types::UnixTime,
        ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
            Ok(rustls::server::danger::ClientCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            m: &[u8],
            c: &rustls::pki_types::CertificateDer<'_>,
            d: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                m,
                c,
                d,
                &self.0.signature_verification_algorithms,
            )
        }
        fn verify_tls13_signature(
            &self,
            m: &[u8],
            c: &rustls::pki_types::CertificateDer<'_>,
            d: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                m,
                c,
                d,
                &self.0.signature_verification_algorithms,
            )
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }

    /// 1 回だけペアリングを受けて、受け取った公開鍵の行を返す。
    fn serve(listener: TcpListener, code: &'static str) -> Option<String> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let (tcp, _) = listener.accept().ok()?;
        let kp = rcgen::KeyPair::generate().ok()?;
        let cert = rcgen::CertificateParams::new(vec!["tv".to_string()])
            .ok()?
            .self_signed(&kp)
            .ok()?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .ok()?
            .with_client_cert_verifier(Arc::new(AnyClient(provider)))
            .with_single_cert(
                vec![CertificateDer::from(cert.der().to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(kp.serialize_der())),
            )
            .ok()?;
        let conn = rustls::ServerConnection::new(Arc::new(config)).ok()?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        while tls.conn.is_handshaking() {
            tls.conn.complete_io(&mut tls.sock).ok()?;
        }
        let mut exported = [0u8; EXPORTER_LEN];
        tls.conn
            .export_keying_material(&mut exported, EXPORTER_LABEL, None)
            .ok()?;
        let mut password = code.as_bytes().to_vec();
        password.extend_from_slice(&exported);
        let b = spake2::Spake2::new(
            spake2::Role::Bob,
            SERVER_NAME,
            CLIENT_NAME,
            &password,
            &[9u8; 64],
        );
        send(&mut tls, 0, b.message()).ok()?;
        let am = recv(&mut tls, 0).ok()?;
        let mut c = cipher::Cipher::new(&b.finish(&am)?);
        let sealed = recv(&mut tls, 1).ok()?;
        let Some(info) = c.open(&sealed) else {
            // adbd と同じく、開けなければ黙って切る
            return None;
        };
        let end = info[1..]
            .iter()
            .position(|&x| x == 0)
            .unwrap_or(info.len() - 1);
        let line = String::from_utf8_lossy(&info[1..1 + end]).into_owned();
        let mut reply = vec![0u8; PEER_INFO_SIZE];
        reply[0] = 1;
        reply[1..13].copy_from_slice(b"adb-FAKE0001");
        send(&mut tls, 1, &c.seal(&reply)).ok()?;
        Some(line)
    }

    fn key_file() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("hushtv-pair-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("adbkey")
    }

    #[test]
    fn pairs_with_the_right_code() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let tv = std::thread::spawn(move || serve(l, "482913"));
        let key = key_file();
        let guid = pair(addr, "482913", &key).unwrap();
        assert_eq!(guid, "adb-FAKE0001");
        let line = tv.join().unwrap().unwrap();
        let pem = std::fs::read_to_string(&key).unwrap();
        assert_eq!(line, public_key_line(&pem).unwrap());
    }

    #[test]
    fn wrong_code_is_reported_as_wrong_code() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let tv = std::thread::spawn(move || serve(l, "111111"));
        let err = pair(addr, "222222", &key_file()).unwrap_err();
        assert!(matches!(err, PairError::WrongCode), "{err:?}");
        assert!(tv.join().unwrap().is_none());
    }
}
