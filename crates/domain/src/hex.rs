//! Codifica esadecimale per chiavi e impronte, senza una dipendenza per 10 righe. (v0.2.0)

const UPPER: &[u8; 16] = b"0123456789ABCDEF";
const LOWER: &[u8; 16] = b"0123456789abcdef";

fn encode(bytes: &[u8], digits: &[u8; 16]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(digits[usize::from(byte >> 4)]));
        out.push(char::from(digits[usize::from(byte & 0x0F)]));
    }
    out
}

pub(crate) fn upper(bytes: &[u8]) -> String {
    encode(bytes, UPPER)
}

pub(crate) fn lower(bytes: &[u8]) -> String {
    encode(bytes, LOWER)
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_both_cases() {
        assert_eq!(super::upper(&[0x00, 0xAB, 0x0F]), "00AB0F");
        assert_eq!(super::lower(&[0x00, 0xAB, 0x0F]), "00ab0f");
    }
}
