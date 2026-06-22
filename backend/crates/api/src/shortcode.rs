use rand::Rng;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// 8-char base62 random code → 218 trillion namespace. Collision probability
/// at our scale is negligible; the create path retries on conflict anyway.
pub fn generate_shortcode() -> String {
    let mut rng = rand::thread_rng();
    let mut buf = [0u8; 8];
    for slot in buf.iter_mut() {
        let idx = rng.gen_range(0..ALPHABET.len());
        *slot = ALPHABET[idx];
    }
    String::from_utf8(buf.to_vec()).expect("alphabet is ASCII")
}
