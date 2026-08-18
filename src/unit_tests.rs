use crate::test::{TestResult, test};

#[test]
fn test_runner_works() -> TestResult {
    TestResult::Pass
}

#[test]
fn simple_addition() -> TestResult {
    if 1 + 1 == 2 {
        return TestResult::Pass;
    } else {
        return TestResult::Fail("lksjdf");
    }
}
