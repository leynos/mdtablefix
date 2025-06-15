/// Utility helpers shared across integration tests.

macro_rules! lines_vec {
    ($($line:expr),* $(,)?) => {
        vec![$($line.to_string()),*]
    };
}
