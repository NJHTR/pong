use pong_core::cas::{digest_for, Cas, CasFailPoint, CasFailpoints, CasFaultAction};
use tempfile::tempdir;

fn staging_count(cas: &Cas) -> usize {
    cas.root()
        .join("staging")
        .read_dir()
        .expect("staging directory")
        .count()
}

#[test]
fn fi08_directory_sync_fault_cold_reopen_verifies_object_and_retry_is_idempotent() {
    let directory = tempdir().expect("directory");
    let payload = b"fi08-cold-reopen-payload";
    let digest = digest_for("snapshot/v1", payload);

    {
        let cas = Cas::new_with_failpoints(
            directory.path(),
            CasFailpoints::once(CasFailPoint::DirectorySync, CasFaultAction::Fail),
        )
        .expect("cas");
        let error = cas
            .put("snapshot/v1", payload)
            .expect_err("directory durability fault");
        assert_eq!(error.code(), "FAULT_INJECTED");
        assert_eq!(cas.failpoints().armed(), None);
        // Publication happened before the injected directory sync failure,
        // but the staging name is no longer reachable.
        assert!(cas.object_path(digest).is_file());
        assert_eq!(staging_count(&cas), 0);
    }

    // Reopen from disk and verify bytes, not merely object-name presence. A
    // retry must observe the immutable object and perform no second publish.
    let reopened = Cas::new(directory.path()).expect("cold reopen");
    assert!(reopened.exists(digest));
    assert_eq!(
        reopened.get("snapshot/v1", digest).expect("object"),
        payload
    );
    assert_eq!(staging_count(&reopened), 0);
    assert_eq!(reopened.put("snapshot/v1", payload).expect("retry"), digest);
    assert_eq!(staging_count(&reopened), 0);
    assert_eq!(
        reopened.get("snapshot/v1", digest).expect("retry object"),
        payload
    );
}
