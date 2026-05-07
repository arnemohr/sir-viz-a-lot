//! Hold an `IOPMAssertion` (macOS) or no-op (other platforms) so the
//! projector display does not sleep mid-show. Drop the value to release.

#[cfg(target_os = "macos")]
mod imp {
    // T-M2-04 / Approach B: declare the IOKit and CoreFoundation C entry
    // points ourselves with `extern "C"` blocks. We deliberately avoid
    // pulling in `core-foundation` / `objc2-core-foundation` as direct deps
    // (forbidden by the task), and `objc2-io-kit` does not re-export the
    // `CFString` type its `IOPMAssertionCreateWithName` binding requires.
    // Both frameworks are already linked transitively via the objc2-* crates,
    // so the `#[link(...)]` attributes below are belt-and-braces — they
    // won't produce duplicate-link errors.

    use core::ffi::c_void;
    use core::ptr;

    type CFStringRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFIndex = isize;
    type CFStringEncoding = u32;
    type Boolean = u8;

    type IOPMAssertionID = u32;
    type IOPMAssertionLevel = u32;
    type IOReturn = i32;

    const K_CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;
    const K_IO_PM_ASSERTION_LEVEL_ON: IOPMAssertionLevel = 255;
    const K_IO_RETURN_SUCCESS: IOReturn = 0;
    /// `kIOPMAssertionTypePreventUserIdleDisplaySleep` from `<IOKit/pwr_mgt/IOPMLib.h>`.
    const ASSERTION_TYPE: &[u8] = b"PreventUserIdleDisplaySleep";

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            num_bytes: CFIndex,
            encoding: CFStringEncoding,
            is_external_representation: Boolean,
        ) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            assertion_level: IOPMAssertionLevel,
            assertion_name: CFStringRef,
            assertion_id: *mut IOPMAssertionID,
        ) -> IOReturn;
        fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
    }

    /// RAII handle for an `IOPMAssertion` of type
    /// `kIOPMAssertionTypePreventUserIdleDisplaySleep`. While alive the
    /// display will not idle-sleep. A `None` `id` means the assertion could
    /// not be acquired (degraded mode); `Drop` is a no-op in that case.
    pub struct SleepAssertion {
        id: Option<IOPMAssertionID>,
    }

    impl SleepAssertion {
        pub fn acquire(reason: &str) -> Self {
            // SAFETY: `CFStringCreateWithBytes` is a CoreFoundation entry
            // point; passing a null allocator selects `kCFAllocatorDefault`.
            // The byte slices outlive the call. We check for null returns.
            let assertion_type = unsafe {
                CFStringCreateWithBytes(
                    ptr::null(),
                    ASSERTION_TYPE.as_ptr(),
                    ASSERTION_TYPE.len() as CFIndex,
                    K_CF_STRING_ENCODING_UTF8,
                    0,
                )
            };
            if assertion_type.is_null() {
                tracing::warn!(
                    "sleep assertion: failed to allocate CFString for assertion type; \
                     display-sleep prevention unavailable (degraded)"
                );
                return Self { id: None };
            }

            let reason_bytes = reason.as_bytes();
            let assertion_name = unsafe {
                CFStringCreateWithBytes(
                    ptr::null(),
                    reason_bytes.as_ptr(),
                    reason_bytes.len() as CFIndex,
                    K_CF_STRING_ENCODING_UTF8,
                    0,
                )
            };
            if assertion_name.is_null() {
                // SAFETY: we just confirmed `assertion_type` is non-null and
                // we own the reference returned by CFStringCreateWithBytes.
                unsafe { CFRelease(assertion_type) };
                tracing::warn!(
                    "sleep assertion: failed to allocate CFString for assertion name; \
                     display-sleep prevention unavailable (degraded)"
                );
                return Self { id: None };
            }

            let mut id: IOPMAssertionID = 0;
            // SAFETY: both CFString refs are non-null and owned; `id` is a
            // valid out-pointer to a stack `u32`. IOKit retains the strings
            // internally — we still own (and must release) our local refs.
            let rc = unsafe {
                IOPMAssertionCreateWithName(
                    assertion_type,
                    K_IO_PM_ASSERTION_LEVEL_ON,
                    assertion_name,
                    &mut id,
                )
            };
            // SAFETY: both CFString refs are non-null and owned by us.
            unsafe {
                CFRelease(assertion_type);
                CFRelease(assertion_name);
            }

            if rc == K_IO_RETURN_SUCCESS {
                tracing::info!(
                    assertion_id = id,
                    reason = reason,
                    "sleep assertion: acquired PreventUserIdleDisplaySleep"
                );
                Self { id: Some(id) }
            } else {
                tracing::warn!(
                    io_return = rc,
                    "sleep assertion: IOPMAssertionCreateWithName failed; \
                     display-sleep prevention unavailable (degraded)"
                );
                Self { id: None }
            }
        }
    }

    impl Drop for SleepAssertion {
        fn drop(&mut self) {
            if let Some(id) = self.id.take() {
                // SAFETY: `id` was returned by a successful
                // IOPMAssertionCreateWithName above and has not been released.
                let rc = unsafe { IOPMAssertionRelease(id) };
                if rc == K_IO_RETURN_SUCCESS {
                    tracing::debug!(
                        assertion_id = id,
                        "sleep assertion: released PreventUserIdleDisplaySleep"
                    );
                } else {
                    tracing::warn!(
                        assertion_id = id,
                        io_return = rc,
                        "sleep assertion: IOPMAssertionRelease returned non-success"
                    );
                }
            }
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
