#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/struct_support.rs");
    t.compile_fail("tests/compilation_errors.rs");
}
