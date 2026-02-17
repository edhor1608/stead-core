use proptest::prelude::*;
use stead_session_model::{BackendKind, build_session_uid};

proptest! {
    #[test]
    fn uid_contains_backend_prefix(id in "[a-zA-Z0-9_-]{1,32}") {
        let uid = build_session_uid(BackendKind::Codex, &id);
        prop_assert!(uid.starts_with("stead:codex:"));
    }

    #[test]
    fn uid_is_deterministic_for_same_input(id in "[a-zA-Z0-9_-]{1,32}") {
        let a = build_session_uid(BackendKind::ClaudeCode, &id);
        let b = build_session_uid(BackendKind::ClaudeCode, &id);
        prop_assert_eq!(a, b);
    }
}
