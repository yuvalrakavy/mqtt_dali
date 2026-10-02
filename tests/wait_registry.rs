//! Every wait in the bridge names a row of `docs/wait-registry.md` (Store no-hang §14.4).

#[test]
fn every_wait_is_registered() {
    wait_lint::assert_registered(env!("CARGO_MANIFEST_DIR"), &["src"], "docs/wait-registry.md");
}
