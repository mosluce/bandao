// Alphabet matches design.md D3: digits 2-9 + uppercase A-Z minus I, O.
// 32 characters total → log2(32^10) ≈ 50 bits of entropy per code.
const ALPHABET: [char; 32] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L',
    'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

const LENGTH: usize = 10;

pub fn generate() -> String {
    nanoid::nanoid!(LENGTH, &ALPHABET)
}

pub fn is_well_formed(code: &str) -> bool {
    code.chars().count() == LENGTH && code.chars().all(|c| ALPHABET.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_code_matches_alphabet() {
        for _ in 0..100 {
            let code = generate();
            assert!(is_well_formed(&code), "bad code: {code}");
        }
    }

    #[test]
    fn rejects_disallowed_chars() {
        assert!(!is_well_formed("0OO0OO0OO0"));
        assert!(!is_well_formed("ABC"));
        assert!(!is_well_formed("IIIIIIIIII"));
    }
}
