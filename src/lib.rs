//! atty is a simple utility that answers one question
//! > is this a tty?
//!
//! usage is just as simple
//!
//! ```
//! if atty::is(atty::Stream::Stdout) {
//!   println!("i'm a tty")
//! }
//! ```
//!
//! ```
//! if atty::isnt(atty::Stream::Stdout) {
//!   println!("i'm not a tty")
//! }
//! ```

#![cfg_attr(unix, no_std)]

#[cfg(unix)]
extern crate libc;
#[cfg(windows)]
extern crate windows_sys;

#[cfg(windows)]
use windows_sys::Win32::System::Console::STD_HANDLE;

/// possible stream sources
#[derive(Clone, Copy, Debug)]
pub enum Stream {
    Stdout,
    Stderr,
    Stdin,
}

/// returns true if this is a tty
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub fn is(stream: Stream) -> bool {
    extern crate libc;

    let fd = match stream {
        Stream::Stdout => libc::STDOUT_FILENO,
        Stream::Stderr => libc::STDERR_FILENO,
        Stream::Stdin => libc::STDIN_FILENO,
    };
    unsafe { libc::isatty(fd) != 0 }
}

/// returns true if this is a tty
#[cfg(target_os = "hermit")]
pub fn is(stream: Stream) -> bool {
    extern crate hermit_abi;

    let fd = match stream {
        Stream::Stdout => hermit_abi::STDOUT_FILENO,
        Stream::Stderr => hermit_abi::STDERR_FILENO,
        Stream::Stdin => hermit_abi::STDIN_FILENO,
    };
    hermit_abi::isatty(fd)
}

/// returns true if this is a tty
#[cfg(windows)]
pub fn is(stream: Stream) -> bool {
    use windows_sys::Win32::System::Console::{
        STD_ERROR_HANDLE as STD_ERROR, STD_INPUT_HANDLE as STD_INPUT,
        STD_OUTPUT_HANDLE as STD_OUTPUT,
    };

    let (fd, others) = match stream {
        Stream::Stdin => (STD_INPUT, [STD_ERROR, STD_OUTPUT]),
        Stream::Stderr => (STD_ERROR, [STD_INPUT, STD_OUTPUT]),
        Stream::Stdout => (STD_OUTPUT, [STD_INPUT, STD_ERROR]),
    };
    if unsafe { console_on_any(&[fd]) } {
        // False positives aren't possible. If we got a console then
        // we definitely have a tty on stdin.
        return true;
    }

    // At this point, we *could* have a false negative. We can determine that
    // this is true negative if we can detect the presence of a console on
    // any of the other streams. If another stream has a console, then we know
    // we're in a Windows console and can therefore trust the negative.
    if unsafe { console_on_any(&others) } {
        return false;
    }

    // Otherwise, we fall back to a very strange msys hack to see if we can
    // sneakily detect the presence of a tty.
    unsafe { msys_tty_on(fd) }
}

/// returns true if this is _not_ a tty
pub fn isnt(stream: Stream) -> bool {
    !is(stream)
}

/// Returns true if any of the given fds are on a console.
#[cfg(windows)]
unsafe fn console_on_any(fds: &[STD_HANDLE]) -> bool {
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle};

    for &fd in fds {
        let mut out = 0;
        let handle = GetStdHandle(fd);
        if GetConsoleMode(handle, &mut out) != 0 {
            return true;
        }
    }
    false
}

/// Returns true if there is an MSYS tty on the given handle.
#[cfg(windows)]
unsafe fn msys_tty_on(fd: STD_HANDLE) -> bool {
    use std::ffi::c_void;
    use windows_sys::Win32::{
        Foundation::MAX_PATH,
        Storage::FileSystem::{FileNameInfo, GetFileInformationByHandleEx},
        System::Console::GetStdHandle,
    };

    /// Mirrors windows_sys::Win32::Storage::FileSystem::FILE_NAME_INFO, giving
    /// it a fixed length that we can stack allocate
    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_NAME_INFO {
        FileNameLength: u32,
        FileName: [u16; MAX_PATH as usize],
    }
    let mut name_info = FILE_NAME_INFO {
        FileNameLength: 0,
        FileName: [0; MAX_PATH as usize],
    };
    let handle = unsafe {
        // Safety: function has no invariants. an invalid handle id will cause
        //         GetFileInformationByHandleEx to return an error
        GetStdHandle(fd)
    };
    let res = unsafe {
        // Safety: handle is valid, and buffer length is fixed
        GetFileInformationByHandleEx(
            handle,
            FileNameInfo,
            &mut name_info as *mut _ as *mut c_void,
            std::mem::size_of::<FILE_NAME_INFO>() as u32,
        )
    };
    if res == 0 {
        return false;
    }
    let s = &name_info.FileName[..name_info.FileNameLength as usize];
    let name = String::from_utf16_lossy(s);
    // This checks whether 'pty' exists in the file name, which indicates that
    // a pseudo-terminal is attached. To mitigate against false positives
    // (e.g., an actual file name that contains 'pty'), we also require that
    // either the strings 'msys-' or 'cygwin-' are in the file name as well.)
    let is_msys = name.contains("msys-") || name.contains("cygwin-");
    let is_pty = name.contains("-pty");
    is_msys && is_pty
}

/// returns true if this is a tty
#[cfg(any(target_arch = "wasm32", target_env = "sgx"))]
pub fn is(_stream: Stream) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{is, Stream};

    #[test]
    #[cfg(windows)]
    fn is_err() {
        // appveyor pipes its output
        assert!(!is(Stream::Stderr))
    }

    #[test]
    #[cfg(windows)]
    fn is_out() {
        // appveyor pipes its output
        assert!(!is(Stream::Stdout))
    }

    #[test]
    #[cfg(windows)]
    fn is_in() {
        assert!(is(Stream::Stdin))
    }

    #[test]
    #[cfg(unix)]
    fn is_err() {
        assert!(is(Stream::Stderr))
    }

    #[test]
    #[cfg(unix)]
    fn is_out() {
        assert!(is(Stream::Stdout))
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn is_in() {
        // macos on travis seems to pipe its input
        assert!(is(Stream::Stdin))
    }

    #[test]
    #[cfg(all(not(target_os = "macos"), unix))]
    fn is_in() {
        assert!(is(Stream::Stdin))
    }
}
