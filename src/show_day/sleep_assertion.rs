//! Hold an `IOPMAssertion` (macOS) or no-op (other platforms) so the
//! projector display does not sleep mid-show. Drop the value to release.

#[cfg(target_os = "macos")]
mod imp {
    pub struct SleepAssertion {
        // TODO(M2): IOPMAssertionID held here, released in Drop.
    }

    impl SleepAssertion {
        pub fn acquire(_reason: &str) -> Self {
            // TODO(M2): IOPMAssertionCreateWithName via objc2-io-kit.
            Self {}
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub struct SleepAssertion;

    impl SleepAssertion {
        pub fn acquire(_reason: &str) -> Self {
            Self
        }
    }
}

pub use imp::SleepAssertion;
