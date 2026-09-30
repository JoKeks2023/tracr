//! macOS `DeviceSensor` (architecture §4.8): idle time from IOKit
//! `HIDIdleTime`, lock state from the CoreGraphics session dictionary, and
//! the kernel's last sleep and wake times from `kern.sleeptime` /
//! `kern.waketime`. None of these needs a privacy permission.

use std::ffi::{CString, c_char, c_void};

use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef, kCFAllocatorDefault};
use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation_sys::number::{
    CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef, CFNumberGetTypeID, CFNumberGetValue,
    CFNumberRef, kCFNumberSInt64Type,
};
use core_foundation_sys::string::{CFStringCreateWithCString, CFStringRef, kCFStringEncodingUTF8};
use tracr_core::device::DeviceSample;
use tracr_core::time::UtcMillis;

use crate::DeviceSensor;

type IoObject = u32;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> IoObject;
    fn IORegistryEntryCreateCFProperty(
        entry: IoObject,
        key: CFStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CFTypeRef;
    fn IOObjectRelease(object: IoObject) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

/// An owned CFString for a static key.
struct CfKey(CFStringRef);

impl CfKey {
    fn new(text: &str) -> Option<Self> {
        let c = CString::new(text).ok()?;
        // SAFETY: `c` is a valid NUL-terminated string for the call's duration.
        let s = unsafe {
            CFStringCreateWithCString(kCFAllocatorDefault, c.as_ptr(), kCFStringEncodingUTF8)
        };
        (!s.is_null()).then_some(Self(s))
    }
}

impl Drop for CfKey {
    fn drop(&mut self) {
        // SAFETY: created by a CF "Create" call; released exactly once.
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}

/// Milliseconds since the last HID input, or `None` if unavailable.
fn hid_idle_ms() -> Option<i64> {
    let name = CString::new("IOHIDSystem").ok()?;
    let key = CfKey::new("HIDIdleTime")?;
    // SAFETY: IOServiceMatching returns a dictionary that IOServiceGetMatchingService
    // consumes; the service object is released below; the property is a CF object
    // we own ("Create" rule) and release after reading.
    unsafe {
        let matching = IOServiceMatching(name.as_ptr());
        if matching.is_null() {
            return None;
        }
        let service = IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return None;
        }
        let prop = IORegistryEntryCreateCFProperty(service, key.0, kCFAllocatorDefault, 0);
        IOObjectRelease(service);
        if prop.is_null() {
            return None;
        }
        let mut ns: i64 = 0;
        let ok = CFGetTypeID(prop) == CFNumberGetTypeID()
            && CFNumberGetValue(
                prop as CFNumberRef,
                kCFNumberSInt64Type,
                &mut ns as *mut i64 as *mut c_void,
            );
        CFRelease(prop);
        ok.then_some(ns / 1_000_000)
    }
}

/// Whether the login session's screen is locked, or `None` without a session.
fn screen_locked() -> Option<bool> {
    let key = CfKey::new("CGSSessionScreenIsLocked")?;
    // SAFETY: the session dictionary is owned ("Copy" rule) and released; the
    // value is borrowed from it and read before the release.
    unsafe {
        let dict = CGSessionCopyCurrentDictionary();
        if dict.is_null() {
            return None;
        }
        let value = CFDictionaryGetValue(dict, key.0 as *const c_void);
        let locked = !value.is_null()
            && CFGetTypeID(value as CFTypeRef) == CFBooleanGetTypeID()
            && CFBooleanGetValue(value as CFBooleanRef);
        CFRelease(dict as CFTypeRef);
        Some(locked)
    }
}

fn sysctl_time(name: &str) -> Option<UtcMillis> {
    let name = CString::new(name).ok()?;
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut size = std::mem::size_of::<libc::timeval>();
    // SAFETY: `tv` and `size` are valid for writes of `size` bytes.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut tv as *mut libc::timeval as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && tv.tv_sec > 0).then(|| UtcMillis(tv.tv_sec * 1000 + i64::from(tv.tv_usec) / 1000))
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacDeviceSensor;

impl DeviceSensor for MacDeviceSensor {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn sample(&self, now: UtcMillis) -> DeviceSample {
        DeviceSample {
            at: now,
            idle_ms: hid_idle_ms(),
            locked: screen_locked(),
            last_sleep_wake: sysctl_time("kern.sleeptime").zip(sysctl_time("kern.waketime")),
        }
    }
}
