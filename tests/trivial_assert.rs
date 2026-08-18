use crate::test::{TestResult, test};

#[test]
fn manual_assert() -> TestResult {
    if 2 + 2 == 4 {
        return TestResult::Pass;
    } else {
        return TestResult::Fail("2 + 2 != 4");
    }
}
