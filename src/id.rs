use chrono::Utc;
use sha2::{Digest, Sha256};

const BASE36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn to_base36(bytes: &[u8], len: usize) -> String {
    let mut result = String::with_capacity(len);
    for i in 0..len {
        let idx = bytes[i % bytes.len()] as usize % 36;
        result.push(BASE36[idx] as char);
    }
    result
}

pub fn generate_id(title: &str, existing_ids: &[&str]) -> String {
    for nonce in 0u32.. {
        let mut hasher = Sha256::new();
        hasher.update(title.as_bytes());
        hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        hasher.update(nonce.to_le_bytes());
        let hash = hasher.finalize();
        let code = to_base36(&hash, 4);
        let id = format!("lb-{code}");
        if !existing_ids.contains(&id.as_str()) {
            return id;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_format() {
        let id = generate_id("some title", &[]);
        assert!(id.starts_with("lb-"), "id should start with lb-: {id}");
        assert_eq!(id.len(), 7, "id should be 7 chars: {id}");
        let suffix = &id[3..];
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "suffix should be base36: {suffix}"
        );
    }

    #[test]
    fn id_uniqueness() {
        let id1 = generate_id("title one", &[]);
        let id2 = generate_id("title two", &[]);
        assert_ne!(id1, id2);
    }

    #[test]
    fn id_collision_avoidance() {
        let first = generate_id("test", &[]);
        let second = generate_id("test", &[first.as_str()]);
        assert_ne!(first, second);
    }

    #[test]
    fn id_empty_title() {
        let id = generate_id("", &[]);
        assert!(id.starts_with("lb-"));
        assert_eq!(id.len(), 7);
    }
}
