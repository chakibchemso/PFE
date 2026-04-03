use ascon_aead::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    AsconAead128, AsconAead128Key, AsconAead128Nonce,
};
use std::cell::RefCell;

pub struct Ascon {
    cipher: AsconAead128,
}

// Client-side cipher instance using thread_local! for WASM context
// This ensures the cipher lives in the client (browser), not on the server
thread_local! {
    static CIPHER: RefCell<Option<Ascon>> = const { RefCell::new(None) };
}

impl Ascon {
    pub fn new(key: &[u8; 16]) -> Self {
        let key = AsconAead128Key::from_slice(key);
        let cipher = <AsconAead128 as KeyInit>::new(key);
        Self { cipher }
    }

    /// Initialize the cipher instance with the provided key (client-side only)
    pub fn init(key: &[u8; 16]) {
        CIPHER.with(|c| {
            *c.borrow_mut() = Some(Self::new(key));
        });
    }

    /// Check if cipher is initialized (client-side only)
    pub fn is_initialized() -> bool {
        CIPHER.with(|c| c.borrow().is_some())
    }

    /// Decrypt using the stored cipher instance (client-side only)
    pub fn decrypt_cached(ciphertext: &[u8], nonce: &[u8; 16]) -> Option<Vec<u8>> {
        CIPHER.with(|c| {
            c.borrow()
                .as_ref()
                .map(|cipher| cipher.decrypt(ciphertext, nonce))
        })
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
