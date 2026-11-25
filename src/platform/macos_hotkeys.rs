use std::ffi::c_void;
use std::sync::Mutex;

use anyhow::{Result, bail};

type OsStatus = i32;

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const SIGNATURE: u32 = u32::from_be_bytes(*b"Kire");
const NO_ERR: OsStatus = 0;
const EVENT_NOT_HANDLED: OsStatus = -9874;

pub const OPTION: u32 = 1 << 11;
pub const SHIFT: u32 = 1 << 9;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetEventDispatcherTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> OsStatus,
        num_types: u32,
        types: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut *mut c_void,
    ) -> OsStatus;
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        out_ref: *mut *mut c_void,
    ) -> OsStatus;
    fn UnregisterEventHotKey(hotkey: *mut c_void) -> OsStatus;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> OsStatus;
}

static PRESSED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static REGISTERED: Mutex<Vec<(u32, usize)>> = Mutex::new(Vec::new());

extern "C" fn on_hotkey(_next: *mut c_void, event: *mut c_void, _data: *mut c_void) -> OsStatus {
    let mut id = EventHotKeyId::default();
    let status = unsafe {
        GetEventParameter(
            event,
            PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyId>(),
            std::ptr::null_mut(),
            (&mut id as *mut EventHotKeyId).cast(),
        )
    };
    if status != NO_ERR || id.signature != SIGNATURE {
        return EVENT_NOT_HANDLED;
    }
    if let Ok(mut pressed) = PRESSED.lock() {
        pressed.push(id.id);
    }
    NO_ERR
}

// O alvo da aplicação (GetApplicationEventTarget, usado pelo crate global-hotkey)
// nunca recebe os atalhos dentro do NSApplication do gpui 0.2.2; o alvo do
// dispatcher recebe.
pub fn install() -> Result<()> {
    let spec = EventTypeSpec {
        event_class: EVENT_CLASS_KEYBOARD,
        event_kind: EVENT_HOT_KEY_PRESSED,
    };
    let mut handler = std::ptr::null_mut();
    let status = unsafe {
        InstallEventHandler(
            GetEventDispatcherTarget(),
            on_hotkey,
            1,
            &spec,
            std::ptr::null_mut(),
            &mut handler,
        )
    };
    if status != NO_ERR {
        bail!("InstallEventHandler falhou ({status})");
    }
    Ok(())
}

pub fn register(id: u32, key_code: u32, modifiers: u32) -> Result<()> {
    let mut hotkey = std::ptr::null_mut();
    let status = unsafe {
        RegisterEventHotKey(
            key_code,
            modifiers,
            EventHotKeyId {
                signature: SIGNATURE,
                id,
            },
            GetEventDispatcherTarget(),
            0,
            &mut hotkey,
        )
    };
    if status != NO_ERR {
        bail!("RegisterEventHotKey falhou ({status}); outro app já usa esse atalho?");
    }
    if let Ok(mut r) = REGISTERED.lock() {
        r.push((id, hotkey as usize));
    }
    Ok(())
}

pub fn unregister(id: u32) {
    if let Ok(mut r) = REGISTERED.lock() {
        r.retain(|(i, h)| {
            if *i == id {
                unsafe { UnregisterEventHotKey(*h as *mut c_void) };
                false
            } else {
                true
            }
        });
    }
}

pub fn unregister_all() {
    if let Ok(mut r) = REGISTERED.lock() {
        for (_, h) in r.drain(..) {
            unsafe { UnregisterEventHotKey(h as *mut c_void) };
        }
    }
}

pub fn drain() -> Vec<u32> {
    PRESSED
        .lock()
        .map(|mut pressed| std::mem::take(&mut *pressed))
        .unwrap_or_default()
}
