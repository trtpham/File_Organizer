#[test]
fn fixtures_dir_exists() {
    assert!(sfo_compat_tests::fixtures_dir().is_dir());
}
