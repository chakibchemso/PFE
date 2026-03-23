use alloc::vec::Vec;
use ascon_aead::{
    AsconAead128, AsconAead128Key, AsconAead128Nonce,
    aead::{Aead, AeadCore, OsRng},
};

pub struct Ascon {
    cipher: AsconAead128,
}

impl Ascon {
    pub fn new(key: &[u8; 16]) -> Self {
        let key = AsconAead128Key::from_slice(key);
        let cipher = <AsconAead128 as ascon_aead::aead::KeyInit>::new(key);
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> (Vec<u8>, [u8; 16]) {
        let nonce = AsconAead128::generate_nonce(&mut OsRng);
        let ciphertext =
            Aead::encrypt(&self.cipher, &nonce, plaintext).expect("encryption failure!");
        (ciphertext, nonce.into())
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8; 16]) -> Vec<u8> {
        let nonce = AsconAead128Nonce::from_slice(nonce);
        Aead::decrypt(&self.cipher, &nonce, ciphertext).expect("decryption failure!")
    }
}
