//! Audio encryption as Plasmo Voice does it (`AesEncryption`): AES-128/CBC/PKCS5Padding with the
//! server's shared key, a fresh random 16-byte IV in front of each packet's ciphertext.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};

type Enc = cbc::Encryptor<aes::Aes128>;
type Dec = cbc::Decryptor<aes::Aes128>;

/// The server's AES key (`ConfigPacket.encryption`, decrypted with the client's RSA key).
#[derive(Clone)]
pub struct AesKey(pub [u8; 16]);

impl std::fmt::Debug for AesKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AesKey(..)")
    }
}

impl AesKey {
    /// From the decrypted key bytes; Plasmo Voice's key is 16 bytes.
    pub fn new(bytes: &[u8]) -> Option<AesKey> {
        Some(AesKey(bytes.try_into().ok()?))
    }

    /// `AesEncryption.encrypt`: `IV ‖ ciphertext`.
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut iv = [0u8; 16];
        // A failing system RNG leaves the IV zero, which still decrypts on the other side.
        let _ = getrandom::fill(&mut iv);
        self.encrypt_with_iv(data, iv)
    }

    pub fn encrypt_with_iv(&self, data: &[u8], iv: [u8; 16]) -> Vec<u8> {
        // No `alloc` feature: pad in a buffer of the padded size.
        let mut buf = vec![0u8; (data.len() / 16 + 1) * 16];
        buf[..data.len()].copy_from_slice(data);
        let ct = match Enc::new(&self.0.into(), &iv.into()).encrypt_padded_mut::<Pkcs7>(&mut buf, data.len()) {
            Ok(ct) => ct.to_vec(),
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::with_capacity(16 + ct.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ct);
        out
    }

    /// `AesEncryption.decrypt`: none when the data is too short or the padding is wrong.
    pub fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 32 || (data.len() - 16) % 16 != 0 {
            return None;
        }
        let iv: [u8; 16] = data[..16].try_into().ok()?;
        let mut buf = data[16..].to_vec();
        Dec::new(&self.0.into(), &iv.into()).decrypt_padded_mut::<Pkcs7>(&mut buf).ok().map(<[u8]>::to_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = AesKey([7; 16]);
        let ct = key.encrypt(b"opus frame");
        assert_eq!(ct.len(), 32);
        assert_eq!(key.decrypt(&ct).unwrap(), b"opus frame");
    }

    #[test]
    fn matches_java_cipher() {
        // From Java 8: Cipher.getInstance("AES/CBC/PKCS5Padding"), key 00..0f, IV 0f..00, "hello".
        let key = AesKey(core::array::from_fn(|i| i as u8));
        let iv: [u8; 16] = core::array::from_fn(|i| 15 - i as u8);
        let ct = key.encrypt_with_iv(b"hello", iv);
        assert_eq!(&ct[..16], &iv);
        let java: Vec<u8> = (0..16).map(|i| u8::from_str_radix(&"181547ce6a5438cb22241856b35cc489"[i * 2..i * 2 + 2], 16).unwrap()).collect();
        assert_eq!(&ct[16..], &java[..]);
        assert_eq!(key.decrypt(&ct).unwrap(), b"hello");
    }
}
