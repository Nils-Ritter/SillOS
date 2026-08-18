use crate::test::{TestResult, test};

#[test]
fn test_int3() -> TestResult {
    x86_64::instructions::interrupts::int3();
    return TestResult::Pass;
}
