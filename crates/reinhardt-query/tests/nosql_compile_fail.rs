#[cfg(feature = "nosql-redis")]
#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/set_nx_xx_conflict.rs");
    t.compile_fail("tests/compile_fail/set_double_expiry.rs");
    t.compile_fail("tests/compile_fail/zadd_nx_gt_conflict.rs");
    t.compile_fail("tests/compile_fail/zadd_gt_lt_conflict.rs");
}
