use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

pub(crate) fn ensure_winrt_initialized() {
    // Many WinRT calls (including `AgileReference::resolve`) require the calling thread to have
    // initialized the Windows Runtime. Calling this multiple times is fine; errors indicate the
    // apartment type was already set up (which is also fine for our purposes).
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

