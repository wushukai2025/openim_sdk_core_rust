use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn gen_operation_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("rust_{millis}_{seq}")
}

pub fn gen_msg_incr(user_id: &str) -> String {
    format!("{user_id}_{}", gen_operation_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msg_incr_keeps_go_prefix_shape() {
        let msg_incr = gen_msg_incr("u1");

        assert!(msg_incr.starts_with("u1_rust_"));
    }
}
