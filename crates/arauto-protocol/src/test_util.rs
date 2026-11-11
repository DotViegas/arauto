//! Helpers shared by the unit tests of this crate.

/// Fails to compile when the type implements any of the listed traits.
///
/// With the trait implemented there are two candidate implementations of the helper trait, and
/// the compiler cannot pick one.
macro_rules! assert_not_impl {
    ($type:ty: $($trait:path),+ $(,)?) => {{
        trait AmbiguousIfImpl<Marker> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
        $({
            #[allow(dead_code)]
            struct Implemented;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Implemented> for T {}
        })+
        let _ = <$type as AmbiguousIfImpl<_>>::check;
    }};
}

/// The seed of every random round in the tests of this crate. It is fixed, so that a failure
/// repeats on the next run; the variable `ARAUTO_TEST_SEED` picks another one.
pub(crate) const TEST_SEED: u64 = 0x4152_4155_544f;

/// The configuration of a test with random rounds: `cases` rounds from a fixed seed, which is
/// printed when the test fails. Nothing is written next to the source.
pub(crate) fn random_rounds(cases: u32) -> proptest::test_runner::Config {
    let seed = std::env::var("ARAUTO_TEST_SEED")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(TEST_SEED);
    // The test harness only shows this line when the test fails.
    println!("random rounds with seed {seed}; set ARAUTO_TEST_SEED to try another");
    proptest::test_runner::Config {
        cases,
        rng_seed: proptest::test_runner::RngSeed::Fixed(seed),
        failure_persistence: None,
        ..proptest::test_runner::Config::default()
    }
}

/// Bytes as lowercase hexadecimal.
pub(crate) fn hex(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}

/// Every way a secret could show up in a text: as it is, in hexadecimal, in base64 (with and
/// without padding, standard and URL alphabets) and in base32.
pub(crate) fn encodings_of(secret: &[u8]) -> Vec<String> {
    let mut forms = vec![
        data_encoding::HEXLOWER.encode(secret),
        data_encoding::HEXUPPER.encode(secret),
        data_encoding::BASE64.encode(secret),
        data_encoding::BASE64_NOPAD.encode(secret),
        data_encoding::BASE64URL.encode(secret),
        data_encoding::BASE64URL_NOPAD.encode(secret),
        data_encoding::BASE32.encode(secret),
        data_encoding::BASE32_NOPAD.encode(secret),
        data_encoding::BASE32_NOPAD
            .encode(secret)
            .to_ascii_lowercase(),
        format!("{secret:?}"),
    ];
    if let Ok(text) = core::str::from_utf8(secret) {
        forms.push(text.to_owned());
    }
    forms
}

/// The form in which `output` shows `secret`, if it shows it in any of the forms of
/// [`encodings_of`].
pub(crate) fn shows(output: &str, secret: &[u8]) -> Option<String> {
    encodings_of(secret)
        .into_iter()
        .find(|form| output.contains(form.as_str()))
}

/// Panics when `output` shows `secret` in any of the forms of [`encodings_of`].
pub(crate) fn assert_hides(output: &str, secret: &[u8], what: &str) {
    if let Some(form) = shows(output, secret) {
        panic!("{what} shows the secret as {form:?}: {output}");
    }
}

/// Lets a test see that a check came before the work it protects. A check that only spares
/// work changes no result, so the work itself reports that it ran: the code calls
/// [`work_probe::record`] under `cfg(test)`, and the test reads the count.
pub(crate) mod work_probe {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static WORK: RefCell<HashMap<&'static str, u32>> = RefCell::new(HashMap::new());
    }

    /// Counts one more run of `work` on this thread.
    pub(crate) fn record(work: &'static str) {
        WORK.with(|counts| {
            let mut counts = counts.borrow_mut();
            let count = counts.entry(work).or_insert(0);
            *count = count.saturating_add(1);
        });
    }

    /// How many times `work` ran on this thread since the last call.
    pub(crate) fn take(work: &'static str) -> u32 {
        WORK.with(|counts| counts.borrow_mut().remove(work).unwrap_or(0))
    }
}
