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
