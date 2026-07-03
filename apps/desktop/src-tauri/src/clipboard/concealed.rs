//! Detects clipboard content marked "concealed" by its source app — the
//! convention password managers (1Password, Bitwarden, KeePass, Dashlane)
//! and the OS's own clipboard history use to say "don't record this."
//! Memora must never capture or sync a password or 2FA code just because a
//! password manager happened to use the system clipboard to deliver it.

#[cfg(windows)]
mod platform {
    use windows::core::HSTRING;
    use windows::Win32::System::DataExchange::{
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };

    /// The de facto Windows standard (used by Windows' own Clipboard History
    /// and every major password manager) for "exclude this clip from
    /// history/monitoring tools." `IsClipboardFormatAvailable` does not
    /// require opening the clipboard first, so this can't contend with the
    /// watcher's own `arboard` reads.
    const EXCLUDE_FORMAT_NAME: &str = "ExcludeClipboardContentFromMonitorProcessing";

    pub fn clipboard_is_concealed() -> bool {
        let name = HSTRING::from(EXCLUDE_FORMAT_NAME);
        let format = unsafe { RegisterClipboardFormatW(&name) };
        if format == 0 {
            tracing::debug!("could not register concealed-clipboard format");
            return false;
        }
        unsafe { IsClipboardFormatAvailable(format).is_ok() }
    }

    #[cfg(test)]
    mod tests {
        use super::clipboard_is_concealed;
        use windows::core::HSTRING;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        };
        use windows::Win32::System::Memory::{GlobalAlloc, GMEM_MOVEABLE};

        /// Reproduces exactly what a password manager does: register the
        /// standard exclude format and mark it present. `SetClipboardData`
        /// requires a real (even if trivial) global memory handle — only
        /// the format's presence is what our detector checks, not content.
        #[test]
        fn detects_the_standard_exclude_format() {
            unsafe {
                OpenClipboard(None).expect("open clipboard");
                EmptyClipboard().expect("empty clipboard");
                let name = HSTRING::from("ExcludeClipboardContentFromMonitorProcessing");
                let format = RegisterClipboardFormatW(&name);
                assert_ne!(format, 0);
                let marker = GlobalAlloc(GMEM_MOVEABLE, 1).expect("alloc marker");
                SetClipboardData(format, HANDLE(marker.0)).expect("mark concealed");
                CloseClipboard().expect("close clipboard");
            }

            assert!(clipboard_is_concealed(), "should detect the exclude format");

            // Leave the clipboard empty rather than concealed after the test.
            unsafe {
                let _ = OpenClipboard(None);
                let _ = EmptyClipboard();
                let _ = CloseClipboard();
            }
            assert!(!clipboard_is_concealed(), "cleared clipboard should not be concealed");
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::atomic::{AtomicBool, Ordering};

    static WARNED: AtomicBool = AtomicBool::new(false);

    /// NOT YET IMPLEMENTED. macOS password managers mark secrets via the
    /// `org.nspasteboard.ConcealedType` NSPasteboard convention, which
    /// requires native AppKit calls (objc2). That code could not be
    /// compiled or verified in this environment (Windows-only sandbox) —
    /// shipping unverified native bindings risked breaking the macOS build
    /// entirely, which is worse than shipping this check Windows-only for
    /// now. Tracked as follow-up work; verify on macOS before implementing.
    pub fn clipboard_is_concealed() -> bool {
        if !WARNED.swap(true, Ordering::Relaxed) {
            tracing::warn!(
                "concealed-clipboard detection is not implemented on macOS yet — \
                 password manager content may be captured. See clipboard/concealed.rs."
            );
        }
        false
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn clipboard_is_concealed() -> bool {
        false
    }
}

pub use platform::clipboard_is_concealed;
