use proptest::test_runner::{Config, FileFailurePersistence, RngAlgorithm, RngSeed};

pub const PURE_CASES: u32 = 10_000;
pub const STORAGE_CASES: u32 = 128;

/// Property inputs are reproducible by default. CI raises every storage-heavy
/// property to the normative 10,000-case corpus; a developer can reproduce a
/// smaller or larger run explicitly with `PONG_PROPTEST_CASES`.
pub fn property_config(seed: u64, local_cases: u32) -> Config {
    let cases = std::env::var("PONG_PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            if std::env::var_os("CI").is_some() {
                PURE_CASES
            } else {
                local_cases
            }
        });

    Config {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        // The default file persistence retains a failing seed. Proptest's
        // normal failure report includes the minimized value and shrink count.
        max_shrink_iters: 16_384,
        verbose: 1,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "proptest-regressions/property_corpus.txt",
        ))),
        ..Config::default()
    }
}
