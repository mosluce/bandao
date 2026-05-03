// Initial password generator for AppUsers. Uses the same alphabet as
// `org_code` (digits 2-9 + uppercase A-Z minus I, O) so admins can dictate
// the password OOB without confusable characters (`0/O`, `1/I/L`).
//
// Length 12 -> log2(32^12) ≈ 60 bits of entropy. Strong enough to resist
// offline attack on the bcrypt hash for the brief window before the user
// changes it. Returned cleartext exactly once on creation / password reset
// and dropped after the response is sent.

const ALPHABET: [char; 32] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L',
    'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

const LENGTH: usize = 12;

pub fn generate_initial() -> String {
    nanoid::nanoid!(LENGTH, &ALPHABET)
}

pub fn is_well_formed(password: &str) -> bool {
    password.chars().count() == LENGTH && password.chars().all(|c| ALPHABET.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_password_matches_alphabet_and_length() {
        for _ in 0..100 {
            let p = generate_initial();
            assert!(is_well_formed(&p), "bad initial password: {p}");
            assert_eq!(p.chars().count(), 12);
        }
    }

    #[test]
    fn rejects_disallowed_chars() {
        // 0 and O are not in the alphabet.
        assert!(!is_well_formed("0OO0OO0OO0OO"));
        assert!(!is_well_formed("ABC"));
    }
}
