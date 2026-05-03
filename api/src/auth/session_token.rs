use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;

const RAW_BYTES: usize = 32;

pub fn generate() -> String {
    let mut buf = [0u8; RAW_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_long_enough() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
        // base64url no-pad of 32 bytes = ceil(32 * 4 / 3) = 43 chars
        assert_eq!(a.len(), 43);
        assert_eq!(b.len(), 43);
    }
}
