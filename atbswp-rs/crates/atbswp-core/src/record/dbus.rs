//! Just enough libdbus (loaded at run time) to talk to the XDG desktop
//! portals: method calls with `a{sv}` options, Request.Response signals,
//! and unix-fd replies.  Mirrors the C implementation in
//! `player/src/backend_linux.c`.

use super::dl::dl_api;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::io::RawFd;
use std::time::{Duration, Instant};

#[repr(C, align(8))]
pub struct DBusMessageIter([u64; 16]); // libdbus: 72 bytes on 64-bit; keep slack
impl DBusMessageIter {
    fn new() -> Self {
        DBusMessageIter([0; 16])
    }
}
#[repr(C)]
pub struct DBusError {
    name: *const c_char,
    message: *const c_char,
    _pad: [u8; 64],
}
impl DBusError {
    fn new() -> Self {
        DBusError {
            name: std::ptr::null(),
            message: std::ptr::null(),
            _pad: [0; 64],
        }
    }
    fn message(&self) -> String {
        if self.message.is_null() {
            "unknown D-Bus error".into()
        } else {
            unsafe { CStr::from_ptr(self.message) }
                .to_string_lossy()
                .into_owned()
        }
    }
}
type Conn = *mut c_void;
type Msg = *mut c_void;

const DBUS_BUS_SESSION: c_int = 0;
pub const T_ARRAY: c_int = b'a' as c_int;
pub const T_DICT_ENTRY: c_int = b'e' as c_int;
pub const T_UNIX_FD: c_int = b'h' as c_int;
pub const T_INT32: c_int = b'i' as c_int;
pub const T_OBJECT_PATH: c_int = b'o' as c_int;
pub const T_STRUCT: c_int = b'r' as c_int;
pub const T_STRING: c_int = b's' as c_int;
pub const T_UINT32: c_int = b'u' as c_int;
pub const T_VARIANT: c_int = b'v' as c_int;

dl_api! {
    pub struct DBus from "libdbus-1.so.3" {
        dbus_error_init: unsafe extern "C" fn(*mut DBusError),
        dbus_error_is_set: unsafe extern "C" fn(*const DBusError) -> c_int,
        dbus_error_free: unsafe extern "C" fn(*mut DBusError),
        dbus_bus_get: unsafe extern "C" fn(c_int, *mut DBusError) -> Conn,
        dbus_bus_get_unique_name: unsafe extern "C" fn(Conn) -> *const c_char,
        dbus_bus_add_match: unsafe extern "C" fn(Conn, *const c_char, *mut DBusError),
        dbus_connection_read_write: unsafe extern "C" fn(Conn, c_int) -> c_int,
        dbus_connection_pop_message: unsafe extern "C" fn(Conn) -> Msg,
        dbus_connection_send_with_reply_and_block: unsafe extern "C" fn(Conn, Msg, c_int, *mut DBusError) -> Msg,
        dbus_message_new_method_call: unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char) -> Msg,
        dbus_message_unref: unsafe extern "C" fn(Msg),
        dbus_message_is_signal: unsafe extern "C" fn(Msg, *const c_char, *const c_char) -> c_int,
        dbus_message_get_path: unsafe extern "C" fn(Msg) -> *const c_char,
        dbus_message_iter_init_append: unsafe extern "C" fn(Msg, *mut DBusMessageIter),
        dbus_message_iter_append_basic: unsafe extern "C" fn(*mut DBusMessageIter, c_int, *const c_void) -> c_int,
        dbus_message_iter_open_container: unsafe extern "C" fn(*mut DBusMessageIter, c_int, *const c_char, *mut DBusMessageIter) -> c_int,
        dbus_message_iter_close_container: unsafe extern "C" fn(*mut DBusMessageIter, *mut DBusMessageIter) -> c_int,
        dbus_message_iter_init: unsafe extern "C" fn(Msg, *mut DBusMessageIter) -> c_int,
        dbus_message_iter_get_arg_type: unsafe extern "C" fn(*mut DBusMessageIter) -> c_int,
        dbus_message_iter_get_basic: unsafe extern "C" fn(*mut DBusMessageIter, *mut c_void),
        dbus_message_iter_recurse: unsafe extern "C" fn(*mut DBusMessageIter, *mut DBusMessageIter),
        dbus_message_iter_next: unsafe extern "C" fn(*mut DBusMessageIter) -> c_int,
    }
}

fn cs(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// A decoded variant value we care about.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    U32(u32),
    I32Pair(i32, i32),
    Streams(Vec<Stream>),
    Other,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Stream {
    pub node_id: u32,
    pub size: Option<(i32, i32)>,
    pub position: Option<(i32, i32)>,
}

pub struct Portal {
    d: DBus,
    conn: Conn,
    sender: String,
    counter: u32,
}

const BUS: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";

impl Portal {
    pub fn connect() -> Result<Portal, String> {
        let d = DBus::load()?;
        let mut err = DBusError::new();
        unsafe { (d.dbus_error_init)(&mut err) };
        let conn = unsafe { (d.dbus_bus_get)(DBUS_BUS_SESSION, &mut err) };
        if conn.is_null() {
            let m = err.message();
            unsafe { (d.dbus_error_free)(&mut err) };
            return Err(format!("cannot connect to the session bus: {m}"));
        }
        let unique = unsafe { CStr::from_ptr((d.dbus_bus_get_unique_name)(conn)) }
            .to_string_lossy()
            .into_owned();
        let sender = unique.trim_start_matches(':').replace('.', "_");
        Ok(Portal {
            d,
            conn,
            sender,
            counter: 0,
        })
    }

    fn add_match(&self, rule: &str) -> Result<(), String> {
        let mut err = DBusError::new();
        unsafe {
            (self.d.dbus_error_init)(&mut err);
            (self.d.dbus_bus_add_match)(self.conn, cs(rule).as_ptr(), &mut err);
            if (self.d.dbus_error_is_set)(&err) != 0 {
                let m = err.message();
                (self.d.dbus_error_free)(&mut err);
                return Err(format!("add_match: {m}"));
            }
        }
        Ok(())
    }

    /// Call `iface.method(args..., a{sv} options)` where `options` gets a
    /// handle_token plus the given entries, and wait for the matching
    /// Request.Response.  Returns (response code, results).
    pub fn request(
        &mut self,
        iface: &str,
        method: &str,
        session: Option<&str>,
        parent_window: bool,
        options: &[(&str, Value)],
        timeout: Duration,
    ) -> Result<(u32, Vec<(String, Value)>), String> {
        self.counter += 1;
        let token = format!("atbswp{}_{}", std::process::id(), self.counter);
        let request_path = format!(
            "/org/freedesktop/portal/desktop/request/{}/{}",
            self.sender, token
        );
        self.add_match(&format!(
            "type='signal',interface='{REQUEST}',member='Response',path='{request_path}'"
        ))?;

        let d = &self.d;
        let msg = unsafe {
            (d.dbus_message_new_method_call)(
                cs(BUS).as_ptr(),
                cs(PATH).as_ptr(),
                cs(iface).as_ptr(),
                cs(method).as_ptr(),
            )
        };
        if msg.is_null() {
            return Err("cannot allocate D-Bus message".into());
        }
        let mut args = DBusMessageIter::new();
        let mut dict = DBusMessageIter::new();
        unsafe {
            (d.dbus_message_iter_init_append)(msg, &mut args);
            if let Some(s) = session {
                let c = cs(s);
                let p = c.as_ptr();
                (d.dbus_message_iter_append_basic)(
                    &mut args,
                    T_OBJECT_PATH,
                    &p as *const _ as *const c_void,
                );
            }
            if parent_window {
                let c = cs("");
                let p = c.as_ptr();
                (d.dbus_message_iter_append_basic)(
                    &mut args,
                    T_STRING,
                    &p as *const _ as *const c_void,
                );
            }
            (d.dbus_message_iter_open_container)(
                &mut args,
                T_ARRAY,
                cs("{sv}").as_ptr(),
                &mut dict,
            );
            self.dict_add(&mut dict, "handle_token", &Value::Str(token.clone()));
            for (k, v) in options {
                self.dict_add(&mut dict, k, v);
            }
            (d.dbus_message_iter_close_container)(&mut args, &mut dict);
        }
        let mut err = DBusError::new();
        unsafe { (d.dbus_error_init)(&mut err) };
        let reply = unsafe {
            (d.dbus_connection_send_with_reply_and_block)(self.conn, msg, 5000, &mut err)
        };
        unsafe { (d.dbus_message_unref)(msg) };
        if reply.is_null() {
            let m = err.message();
            unsafe { (d.dbus_error_free)(&mut err) };
            return Err(format!("{method}: {m}"));
        }
        // Older portals may answer with a different request path.
        let mut alt_path = String::new();
        let mut rit = DBusMessageIter::new();
        unsafe {
            if (d.dbus_message_iter_init)(reply, &mut rit) != 0
                && (d.dbus_message_iter_get_arg_type)(&mut rit) == T_OBJECT_PATH
            {
                let mut p: *const c_char = std::ptr::null();
                (d.dbus_message_iter_get_basic)(&mut rit, &mut p as *mut _ as *mut c_void);
                let h = CStr::from_ptr(p).to_string_lossy().into_owned();
                if h != request_path {
                    alt_path = h;
                }
            }
            (d.dbus_message_unref)(reply);
        }
        if !alt_path.is_empty() {
            self.add_match(&format!(
                "type='signal',interface='{REQUEST}',member='Response',path='{alt_path}'"
            ))?;
        }

        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if unsafe { (d.dbus_connection_read_write)(self.conn, 100) } == 0 {
                return Err("session bus connection closed".into());
            }
            loop {
                let m = unsafe { (d.dbus_connection_pop_message)(self.conn) };
                if m.is_null() {
                    break;
                }
                let is_resp = unsafe {
                    (d.dbus_message_is_signal)(m, cs(REQUEST).as_ptr(), cs("Response").as_ptr())
                } != 0;
                let path = unsafe {
                    let p = (d.dbus_message_get_path)(m);
                    if p.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(p).to_string_lossy().into_owned()
                    }
                };
                if is_resp && (path == request_path || (!alt_path.is_empty() && path == alt_path)) {
                    let r = self.parse_response(m);
                    unsafe { (d.dbus_message_unref)(m) };
                    return r;
                }
                unsafe { (d.dbus_message_unref)(m) };
            }
        }
        Err(format!("timed out waiting for the {method} response"))
    }

    unsafe fn dict_add(&self, dict: &mut DBusMessageIter, key: &str, v: &Value) {
        let d = &self.d;
        let mut entry = DBusMessageIter::new();
        let mut var = DBusMessageIter::new();
        let k = cs(key);
        let kp = k.as_ptr();
        unsafe {
            (d.dbus_message_iter_open_container)(dict, T_DICT_ENTRY, std::ptr::null(), &mut entry);
            (d.dbus_message_iter_append_basic)(
                &mut entry,
                T_STRING,
                &kp as *const _ as *const c_void,
            );
            match v {
                Value::Str(s) => {
                    let c = cs(s);
                    let p = c.as_ptr();
                    (d.dbus_message_iter_open_container)(
                        &mut entry,
                        T_VARIANT,
                        cs("s").as_ptr(),
                        &mut var,
                    );
                    (d.dbus_message_iter_append_basic)(
                        &mut var,
                        T_STRING,
                        &p as *const _ as *const c_void,
                    );
                }
                Value::U32(u) => {
                    (d.dbus_message_iter_open_container)(
                        &mut entry,
                        T_VARIANT,
                        cs("u").as_ptr(),
                        &mut var,
                    );
                    (d.dbus_message_iter_append_basic)(
                        &mut var,
                        T_UINT32,
                        u as *const u32 as *const c_void,
                    );
                }
                _ => unreachable!("unsupported option type"),
            }
            (d.dbus_message_iter_close_container)(&mut entry, &mut var);
            (d.dbus_message_iter_close_container)(dict, &mut entry);
        }
    }

    fn parse_response(&self, m: Msg) -> Result<(u32, Vec<(String, Value)>), String> {
        let d = &self.d;
        let mut it = DBusMessageIter::new();
        unsafe {
            if (d.dbus_message_iter_init)(m, &mut it) == 0
                || (d.dbus_message_iter_get_arg_type)(&mut it) != T_UINT32
            {
                return Err("malformed Response signal".into());
            }
            let mut code: u32 = 0;
            (d.dbus_message_iter_get_basic)(&mut it, &mut code as *mut u32 as *mut c_void);
            let mut results = vec![];
            if (d.dbus_message_iter_next)(&mut it) != 0
                && (d.dbus_message_iter_get_arg_type)(&mut it) == T_ARRAY
            {
                let mut dict = DBusMessageIter::new();
                (d.dbus_message_iter_recurse)(&mut it, &mut dict);
                while (d.dbus_message_iter_get_arg_type)(&mut dict) == T_DICT_ENTRY {
                    let mut entry = DBusMessageIter::new();
                    (d.dbus_message_iter_recurse)(&mut dict, &mut entry);
                    if (d.dbus_message_iter_get_arg_type)(&mut entry) == T_STRING {
                        let mut kp: *const c_char = std::ptr::null();
                        (d.dbus_message_iter_get_basic)(
                            &mut entry,
                            &mut kp as *mut _ as *mut c_void,
                        );
                        let key = CStr::from_ptr(kp).to_string_lossy().into_owned();
                        if (d.dbus_message_iter_next)(&mut entry) != 0
                            && (d.dbus_message_iter_get_arg_type)(&mut entry) == T_VARIANT
                        {
                            let mut var = DBusMessageIter::new();
                            (d.dbus_message_iter_recurse)(&mut entry, &mut var);
                            results.push((key, self.read_value(&mut var)));
                        }
                    }
                    (d.dbus_message_iter_next)(&mut dict);
                }
            }
            Ok((code, results))
        }
    }

    unsafe fn read_value(&self, it: &mut DBusMessageIter) -> Value {
        let d = &self.d;
        unsafe {
            match (d.dbus_message_iter_get_arg_type)(it) {
                T_STRING | T_OBJECT_PATH => {
                    let mut p: *const c_char = std::ptr::null();
                    (d.dbus_message_iter_get_basic)(it, &mut p as *mut _ as *mut c_void);
                    Value::Str(CStr::from_ptr(p).to_string_lossy().into_owned())
                }
                T_UINT32 => {
                    let mut u: u32 = 0;
                    (d.dbus_message_iter_get_basic)(it, &mut u as *mut u32 as *mut c_void);
                    Value::U32(u)
                }
                T_STRUCT => {
                    let mut s = DBusMessageIter::new();
                    (d.dbus_message_iter_recurse)(it, &mut s);
                    let mut a: i32 = 0;
                    let mut b: i32 = 0;
                    if (d.dbus_message_iter_get_arg_type)(&mut s) == T_INT32 {
                        (d.dbus_message_iter_get_basic)(&mut s, &mut a as *mut i32 as *mut c_void);
                        if (d.dbus_message_iter_next)(&mut s) != 0
                            && (d.dbus_message_iter_get_arg_type)(&mut s) == T_INT32
                        {
                            (d.dbus_message_iter_get_basic)(
                                &mut s,
                                &mut b as *mut i32 as *mut c_void,
                            );
                            return Value::I32Pair(a, b);
                        }
                    }
                    Value::Other
                }
                T_ARRAY => {
                    // a(ua{sv}): the ScreenCast streams list
                    let mut arr = DBusMessageIter::new();
                    (d.dbus_message_iter_recurse)(it, &mut arr);
                    let mut streams = vec![];
                    while (d.dbus_message_iter_get_arg_type)(&mut arr) == T_STRUCT {
                        let mut st = DBusMessageIter::new();
                        (d.dbus_message_iter_recurse)(&mut arr, &mut st);
                        let mut s = Stream::default();
                        if (d.dbus_message_iter_get_arg_type)(&mut st) == T_UINT32 {
                            (d.dbus_message_iter_get_basic)(
                                &mut st,
                                &mut s.node_id as *mut u32 as *mut c_void,
                            );
                            if (d.dbus_message_iter_next)(&mut st) != 0
                                && (d.dbus_message_iter_get_arg_type)(&mut st) == T_ARRAY
                            {
                                let mut props = DBusMessageIter::new();
                                (d.dbus_message_iter_recurse)(&mut st, &mut props);
                                while (d.dbus_message_iter_get_arg_type)(&mut props) == T_DICT_ENTRY
                                {
                                    let mut e = DBusMessageIter::new();
                                    (d.dbus_message_iter_recurse)(&mut props, &mut e);
                                    let mut kp: *const c_char = std::ptr::null();
                                    (d.dbus_message_iter_get_basic)(
                                        &mut e,
                                        &mut kp as *mut _ as *mut c_void,
                                    );
                                    let key = CStr::from_ptr(kp).to_string_lossy().into_owned();
                                    if (d.dbus_message_iter_next)(&mut e) != 0 {
                                        let mut var = DBusMessageIter::new();
                                        (d.dbus_message_iter_recurse)(&mut e, &mut var);
                                        match (key.as_str(), self.read_value(&mut var)) {
                                            ("size", Value::I32Pair(w, h)) => s.size = Some((w, h)),
                                            ("position", Value::I32Pair(x, y)) => {
                                                s.position = Some((x, y))
                                            }
                                            _ => {}
                                        }
                                    }
                                    (d.dbus_message_iter_next)(&mut props);
                                }
                            }
                        }
                        streams.push(s);
                        (d.dbus_message_iter_next)(&mut arr);
                    }
                    Value::Streams(streams)
                }
                _ => Value::Other,
            }
        }
    }

    /// Plain method call returning a unix fd: `iface.method(o session, a{sv} {})`.
    pub fn call_for_fd(
        &mut self,
        iface: &str,
        method: &str,
        session: &str,
    ) -> Result<RawFd, String> {
        let d = &self.d;
        let msg = unsafe {
            (d.dbus_message_new_method_call)(
                cs(BUS).as_ptr(),
                cs(PATH).as_ptr(),
                cs(iface).as_ptr(),
                cs(method).as_ptr(),
            )
        };
        let mut args = DBusMessageIter::new();
        let mut dict = DBusMessageIter::new();
        unsafe {
            (d.dbus_message_iter_init_append)(msg, &mut args);
            let c = cs(session);
            let p = c.as_ptr();
            (d.dbus_message_iter_append_basic)(
                &mut args,
                T_OBJECT_PATH,
                &p as *const _ as *const c_void,
            );
            (d.dbus_message_iter_open_container)(
                &mut args,
                T_ARRAY,
                cs("{sv}").as_ptr(),
                &mut dict,
            );
            (d.dbus_message_iter_close_container)(&mut args, &mut dict);
        }
        let mut err = DBusError::new();
        unsafe { (d.dbus_error_init)(&mut err) };
        let reply = unsafe {
            (d.dbus_connection_send_with_reply_and_block)(self.conn, msg, 5000, &mut err)
        };
        unsafe { (d.dbus_message_unref)(msg) };
        if reply.is_null() {
            let m = err.message();
            unsafe { (d.dbus_error_free)(&mut err) };
            return Err(format!("{method}: {m}"));
        }
        let mut fd: c_int = -1;
        let mut rit = DBusMessageIter::new();
        unsafe {
            if (d.dbus_message_iter_init)(reply, &mut rit) != 0
                && (d.dbus_message_iter_get_arg_type)(&mut rit) == T_UNIX_FD
            {
                (d.dbus_message_iter_get_basic)(&mut rit, &mut fd as *mut c_int as *mut c_void);
            }
            (d.dbus_message_unref)(reply);
        }
        if fd < 0 {
            Err(format!("{method} returned no file descriptor"))
        } else {
            Ok(fd)
        }
    }
}
