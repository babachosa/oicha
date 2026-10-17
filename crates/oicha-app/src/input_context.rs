//! Notify external keyboard-hook input methods when an application command
//! changes the editing context without sending a native caret-navigation key.

#[cfg(target_os = "windows")]
mod commands {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use windows::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, GetKeyboardState, SetKeyboardState, VK_CONTROL, VK_LSHIFT,
                VK_LWIN, VK_MENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
            },
            WindowsAndMessaging::{
                CallNextHookEx, GetForegroundWindow, HHOOK, KBDLLHOOKSTRUCT, SendMessageW,
                SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
            },
        },
    };

    struct State {
        hwnd: HWND,
        enabled: bool,
        held: [bool; 256],
    }
    thread_local! {
        static STATE: RefCell<State> = const { RefCell::new(State {
            hwnd: HWND(std::ptr::null_mut()), enabled: false, held: [false; 256],
        }) };
    }

    pub struct InputHook(HHOOK);
    impl Drop for InputHook {
        fn drop(&mut self) {
            let _ = unsafe { UnhookWindowsHookEx(self.0) };
        }
    }

    pub fn install() -> Option<InputHook> {
        let module = unsafe { GetModuleHandleW(None) }.ok()?;
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), Some(module.into()), 0) } {
            Ok(hook) => Some(InputHook(hook)),
            Err(error) => {
                log::error!("Could not isolate Vim commands from keyboard input methods: {error}");
                None
            }
        }
    }

    pub fn set_mode(window: &gpui::Window, enabled: bool) {
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(handle) = handle.as_raw()
        {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.hwnd = HWND(handle.hwnd.get() as *mut _);
                state.enabled = enabled;
            });
        }
    }

    unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == 0 {
            // Windows supplies this structure for the duration of the callback.
            let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = key.vkCode as usize;
            let message = wparam.0 as u32;
            if vk < 256 && matches!(message, WM_KEYDOWN | WM_KEYUP) {
                let foreground = unsafe { GetForegroundWindow() };
                let target = STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    if message == WM_KEYUP {
                        let held = std::mem::take(&mut state.held[vk]);
                        return (held && state.hwnd == foreground).then_some((state.hwnd, true));
                    }
                    if state.hwnd != foreground {
                        return None;
                    }
                    let modified = [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
                        .iter()
                        .any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0);
                    let command_key = matches!(vk, 0x30..=0x5a | 0xba..=0xc0 | 0xdb..=0xdf | 0xe2);
                    if state.enabled && command_key && !modified {
                        let repeated = std::mem::replace(&mut state.held[vk], true);
                        Some((state.hwnd, repeated))
                    } else {
                        None
                    }
                });
                if let Some((hwnd, repeated)) = target {
                    let mut flags = 1 | ((key.scanCode as isize & 0xff) << 16);
                    if key.flags.0 & 1 != 0 {
                        flags |= 1 << 24;
                    }
                    if repeated {
                        flags |= 1 << 30;
                    }
                    if message == WM_KEYUP {
                        flags |= 1isize << 31;
                    }
                    // Dispatch on the installing UI thread before returning, so
                    // i/c/o switch to Insert before the next physical key. No
                    // app/entity borrow is held across this native callback.
                    // Only a focused Vim editor's command keys bypass the IME;
                    // Insert, prompts, shortcuts and other apps use the OS path.
                    // A hook runs before the queue updates GetKeyState. Reflect
                    // physical Shift while dispatching, otherwise fast Shift+G/A
                    // can be decoded as lowercase. Restore the queue state after.
                    let mut original = [0u8; 256];
                    let saved = unsafe { GetKeyboardState(&mut original) }.is_ok();
                    if saved {
                        let mut current = original;
                        for vk in [VK_SHIFT, VK_LSHIFT, VK_RSHIFT] {
                            current[vk.0 as usize] = (current[vk.0 as usize] & 0x7f)
                                | if unsafe { GetAsyncKeyState(vk.0 as i32) } < 0 {
                                    0x80
                                } else {
                                    0
                                };
                        }
                        let _ = unsafe { SetKeyboardState(&current) };
                    }
                    unsafe {
                        SendMessageW(hwnd, message, Some(WPARAM(vk)), Some(LPARAM(flags)));
                    }
                    if saved {
                        let _ = unsafe { SetKeyboardState(&original) };
                    }
                    return LRESULT(1);
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
pub use commands::{install, set_mode};
#[cfg(not(target_os = "windows"))]
pub fn install() {}
#[cfg(not(target_os = "windows"))]
pub fn set_mode(_: &gpui::Window, _: bool) {}

#[cfg(target_os = "windows")]
pub fn finish_vim_command(window: &gpui::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_F24,
            },
            WindowsAndMessaging::GetForegroundWindow,
        },
    };

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    // SendInput is system-wide. Never send a context boundary to another app
    // (including a different Oicha window) when focus has moved away.
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    if unsafe { GetForegroundWindow() } != hwnd {
        return;
    }
    // A function key ends UniKey's spelling buffer without inserting a space,
    // moving the caret, or changing the user's Vietnamese/English setting.
    // F24 is ignored by our dispatch tree and cannot recurse into this method.
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F24,
                ..Default::default()
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F24,
                dwFlags: KEYEVENTF_KEYUP,
                ..Default::default()
            },
        },
    };
    if unsafe { SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32) } != 2 {
        log::warn!("Could not notify keyboard input method of Vim context change");
    }
}

#[cfg(not(target_os = "windows"))]
pub fn finish_vim_command(_: &gpui::Window) {}
