//! Watching one key whether or not our window has the focus, for push to
//! talk. Only Windows can do this; elsewhere the key never reads as down.

/// Whether the key with this virtual key code is held right now.
#[cfg(windows)]
pub fn is_down(code: u16) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // SAFETY: a plain query of the keyboard state with no pointers.
    // The top bit of the result says the key is down.
    unsafe { GetAsyncKeyState(i32::from(code)) < 0 }
}

#[cfg(not(windows))]
pub fn is_down(_code: u16) -> bool {
    false
}
