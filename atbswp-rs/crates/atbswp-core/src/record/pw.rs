//! Cursor positions from a PipeWire screen-cast stream, using only the
//! `spa_meta_cursor` metadata attached to each buffer (pixels are never
//! mapped).  `libpipewire-0.3.so.0` is loaded at run time; the constants and
//! struct layouts below are the stable SPA/PipeWire ABI, checked against the
//! real headers by `tests/pw_abi_check.c` in CI.

use super::dl::dl_api;
#[allow(unused_imports)]
use c_int as _c_int;
use std::ffi::{c_char, c_int, c_void};
use std::os::unix::io::RawFd;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

// ---- ABI ---------------------------------------------------------------
pub const SPA_META_CURSOR: u32 = 5;
pub const SPA_TYPE_ID: u32 = 3;
pub const SPA_TYPE_INT: u32 = 4;
pub const SPA_TYPE_RECTANGLE: u32 = 10;
pub const SPA_TYPE_FRACTION: u32 = 11;
pub const SPA_TYPE_OBJECT: u32 = 15;
pub const SPA_TYPE_CHOICE: u32 = 19;
pub const SPA_CHOICE_RANGE: u32 = 1;
pub const SPA_CHOICE_ENUM: u32 = 3;
pub const SPA_TYPE_OBJECT_FORMAT: u32 = 262147;
pub const SPA_TYPE_OBJECT_PARAM_META: u32 = 262149;
pub const SPA_PARAM_ENUM_FORMAT: u32 = 3;
pub const SPA_PARAM_FORMAT: u32 = 4;
pub const SPA_PARAM_META: u32 = 6;
pub const SPA_PARAM_META_TYPE: u32 = 1;
pub const SPA_PARAM_META_SIZE: u32 = 2;
pub const SPA_FORMAT_MEDIA_TYPE: u32 = 1;
pub const SPA_FORMAT_MEDIA_SUBTYPE: u32 = 2;
pub const SPA_FORMAT_VIDEO_FORMAT: u32 = 131073;
pub const SPA_FORMAT_VIDEO_SIZE: u32 = 131075;
pub const SPA_FORMAT_VIDEO_FRAMERATE: u32 = 131076;
pub const SPA_MEDIA_TYPE_VIDEO: u32 = 2;
pub const SPA_MEDIA_SUBTYPE_RAW: u32 = 1;
pub const SPA_VIDEO_FORMAT_RGBX: u32 = 7;
pub const SPA_VIDEO_FORMAT_BGRX: u32 = 8;
pub const SPA_VIDEO_FORMAT_RGBA: u32 = 11;
pub const SPA_VIDEO_FORMAT_BGRA: u32 = 12;
pub const SPA_VIDEO_FORMAT_RGB: u32 = 15;
pub const SPA_VIDEO_FORMAT_BGR: u32 = 16;
pub const PW_DIRECTION_INPUT: u32 = 0;
pub const PW_STREAM_FLAG_AUTOCONNECT: u32 = 1;
pub const PW_STREAM_STATE_ERROR: u32 = 0xFFFF_FFFF;
pub const SIZEOF_SPA_HOOK: usize = 48;
pub const SIZEOF_SPA_META_CURSOR: usize = 28;
pub const SIZEOF_SPA_META_BITMAP: usize = 20;

#[repr(C)]
pub struct SpaMeta {
    pub type_: u32,
    pub size: u32,
    pub data: *mut c_void,
}
#[repr(C)]
pub struct SpaBuffer {
    pub n_metas: u32,
    pub n_datas: u32,
    pub metas: *mut SpaMeta,
    pub datas: *mut c_void,
}
#[repr(C)]
pub struct PwBuffer {
    pub buffer: *mut SpaBuffer,
    pub user_data: *mut c_void,
    pub size: u64,
    pub requested: u64,
    pub time: u64,
}
#[repr(C)]
pub struct SpaMetaCursor {
    pub id: u32,
    pub flags: u32,
    pub x: i32,
    pub y: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
    pub bitmap_offset: u32,
}
/// `struct pw_stream_events`, version 0 (the fields we use exist since 0.3.0).
#[repr(C)]
pub struct PwStreamEvents {
    pub version: u32,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub state_changed: Option<unsafe extern "C" fn(*mut c_void, u32, u32, *const c_char)>,
    pub control_info: Option<unsafe extern "C" fn(*mut c_void, u32, *const c_void)>,
    pub io_changed: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void, u32)>,
    pub param_changed: Option<unsafe extern "C" fn(*mut c_void, u32, *const u8)>,
    pub add_buffer: Option<unsafe extern "C" fn(*mut c_void, *mut PwBuffer)>,
    pub remove_buffer: Option<unsafe extern "C" fn(*mut c_void, *mut PwBuffer)>,
    pub process: Option<unsafe extern "C" fn(*mut c_void)>,
    pub drained: Option<unsafe extern "C" fn(*mut c_void)>,
    pub command: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    pub trigger_done: Option<unsafe extern "C" fn(*mut c_void)>,
}
#[repr(C, align(8))]
pub struct SpaHook([u8; SIZEOF_SPA_HOOK]);

dl_api! {
    pub struct Pw from "libpipewire-0.3.so.0" {
        pw_init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char),
        pw_main_loop_new: unsafe extern "C" fn(*const c_void) -> *mut c_void,
        pw_main_loop_get_loop: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        pw_main_loop_run: unsafe extern "C" fn(*mut c_void) -> c_int,
        pw_main_loop_quit: unsafe extern "C" fn(*mut c_void) -> c_int,
        pw_main_loop_destroy: unsafe extern "C" fn(*mut c_void),
        pw_context_new: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void,
        pw_context_connect_fd: unsafe extern "C" fn(*mut c_void, c_int, *mut c_void, usize) -> *mut c_void,
        pw_context_destroy: unsafe extern "C" fn(*mut c_void),
        pw_core_disconnect: unsafe extern "C" fn(*mut c_void) -> c_int,
        pw_properties_new: unsafe extern "C" fn(*const c_char, ...) -> *mut c_void,
        pw_stream_new: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void) -> *mut c_void,
        pw_stream_destroy: unsafe extern "C" fn(*mut c_void),
        pw_stream_add_listener: unsafe extern "C" fn(*mut c_void, *mut SpaHook, *const PwStreamEvents, *mut c_void),
        pw_stream_connect: unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut *const u8, u32) -> c_int,
        pw_stream_update_params: unsafe extern "C" fn(*mut c_void, *mut *const u8, u32) -> c_int,
        pw_stream_dequeue_buffer: unsafe extern "C" fn(*mut c_void) -> *mut PwBuffer,
        pw_stream_queue_buffer: unsafe extern "C" fn(*mut c_void, *mut PwBuffer) -> c_int,
    }
}

// ---- SPA POD building/parsing -------------------------------------------

/// Serialises SPA PODs: `{u32 size, u32 type}` headers, bodies padded to 8.
pub struct Pod;

impl Pod {
    fn pad(v: &mut Vec<u8>) {
        while !v.len().is_multiple_of(8) {
            v.push(0);
        }
    }
    fn prim(ty: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&ty.to_le_bytes());
        v.extend_from_slice(body);
        Self::pad(&mut v);
        v
    }
    pub fn id(v: u32) -> Vec<u8> {
        Self::prim(SPA_TYPE_ID, &v.to_le_bytes())
    }
    pub fn int(v: i32) -> Vec<u8> {
        Self::prim(SPA_TYPE_INT, &v.to_le_bytes())
    }
    pub fn rect(w: u32, h: u32) -> Vec<u8> {
        Self::prim(
            SPA_TYPE_RECTANGLE,
            &[w.to_le_bytes(), h.to_le_bytes()].concat(),
        )
    }
    pub fn frac(n: u32, d: u32) -> Vec<u8> {
        Self::prim(
            SPA_TYPE_FRACTION,
            &[n.to_le_bytes(), d.to_le_bytes()].concat(),
        )
    }
    /// Choice over values of one primitive type (each given as a full pod).
    pub fn choice(kind: u32, values: &[Vec<u8>]) -> Vec<u8> {
        let child_size = u32::from_le_bytes(values[0][0..4].try_into().unwrap());
        let child_type = u32::from_le_bytes(values[0][4..8].try_into().unwrap());
        let mut body = vec![];
        body.extend_from_slice(&kind.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // flags
        body.extend_from_slice(&child_size.to_le_bytes());
        body.extend_from_slice(&child_type.to_le_bytes());
        for v in values {
            body.extend_from_slice(&v[8..8 + child_size as usize]);
        }
        Self::prim(SPA_TYPE_CHOICE, &body)
    }
    /// Object with `(key, pod)` properties.
    pub fn object(obj_type: u32, id: u32, props: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut body = vec![];
        body.extend_from_slice(&obj_type.to_le_bytes());
        body.extend_from_slice(&id.to_le_bytes());
        for (k, p) in props {
            body.extend_from_slice(&k.to_le_bytes());
            body.extend_from_slice(&0u32.to_le_bytes()); // flags
            body.extend_from_slice(p);
        }
        Self::prim(SPA_TYPE_OBJECT, &body)
    }

    /// The video format we are willing to accept: any common RGB layout, any
    /// size, any frame rate.  We never read the pixels.
    pub fn enum_format() -> Vec<u8> {
        Self::object(
            SPA_TYPE_OBJECT_FORMAT,
            SPA_PARAM_ENUM_FORMAT,
            &[
                (SPA_FORMAT_MEDIA_TYPE, Self::id(SPA_MEDIA_TYPE_VIDEO)),
                (SPA_FORMAT_MEDIA_SUBTYPE, Self::id(SPA_MEDIA_SUBTYPE_RAW)),
                (
                    SPA_FORMAT_VIDEO_FORMAT,
                    Self::choice(
                        SPA_CHOICE_ENUM,
                        &[
                            Self::id(SPA_VIDEO_FORMAT_BGRX),
                            Self::id(SPA_VIDEO_FORMAT_BGRX),
                            Self::id(SPA_VIDEO_FORMAT_RGBX),
                            Self::id(SPA_VIDEO_FORMAT_BGRA),
                            Self::id(SPA_VIDEO_FORMAT_RGBA),
                            Self::id(SPA_VIDEO_FORMAT_RGB),
                            Self::id(SPA_VIDEO_FORMAT_BGR),
                        ],
                    ),
                ),
                (
                    SPA_FORMAT_VIDEO_SIZE,
                    Self::choice(
                        SPA_CHOICE_RANGE,
                        &[
                            Self::rect(320, 240),
                            Self::rect(1, 1),
                            Self::rect(16384, 16384),
                        ],
                    ),
                ),
                (
                    SPA_FORMAT_VIDEO_FRAMERATE,
                    Self::choice(
                        SPA_CHOICE_RANGE,
                        &[Self::frac(25, 1), Self::frac(0, 1), Self::frac(1000, 1)],
                    ),
                ),
            ],
        )
    }

    /// Ask the producer to attach cursor metadata to every buffer.
    pub fn meta_cursor() -> Vec<u8> {
        let size = |w: usize, h: usize| {
            (SIZEOF_SPA_META_CURSOR + SIZEOF_SPA_META_BITMAP + w * h * 4) as i32
        };
        Self::object(
            SPA_TYPE_OBJECT_PARAM_META,
            SPA_PARAM_META,
            &[
                (SPA_PARAM_META_TYPE, Self::id(SPA_META_CURSOR)),
                (
                    SPA_PARAM_META_SIZE,
                    Self::choice(
                        SPA_CHOICE_RANGE,
                        &[
                            Self::int(size(64, 64)),
                            Self::int(size(1, 1)),
                            Self::int(size(1024, 1024)),
                        ],
                    ),
                ),
            ],
        )
    }

    /// Read `SPA_FORMAT_VIDEO_size` out of a negotiated Format object.
    pub unsafe fn format_size(pod: *const u8) -> Option<(u32, u32)> {
        unsafe {
            let rd = |p: *const u8| {
                u32::from_le_bytes(std::slice::from_raw_parts(p, 4).try_into().unwrap())
            };
            let size = rd(pod) as usize;
            if rd(pod.add(4)) != SPA_TYPE_OBJECT || size < 8 {
                return None;
            }
            let end = pod.add(8 + size);
            let mut p = pod.add(16); // skip object type + id
            while p.add(16) <= end {
                let key = rd(p);
                let vsize = rd(p.add(8)) as usize;
                let vtype = rd(p.add(12));
                let mut val = p.add(16);
                let mut val_type = vtype;
                if vtype == SPA_TYPE_CHOICE && vsize >= 16 {
                    // choice body: kind, flags, child size, child type, values...
                    val_type = rd(val.add(12));
                    val = val.add(16);
                }
                if key == SPA_FORMAT_VIDEO_SIZE && val_type == SPA_TYPE_RECTANGLE {
                    return Some((rd(val), rd(val.add(4))));
                }
                p = p.add(16 + vsize.div_ceil(8) * 8);
            }
            None
        }
    }
}

// ---- the stream ---------------------------------------------------------

pub struct Sample {
    pub t_us: u64,
    pub x: i32,
    pub y: i32,
}

struct State {
    pw: Pw,
    stream: *mut c_void,
    main_loop: *mut c_void,
    tx: Sender<Sample>,
    size: Arc<Mutex<Option<(u32, u32)>>>,
    error: Arc<Mutex<Option<String>>>,
    meta_pod: Vec<u8>,
}

fn now_us() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1000
}

unsafe extern "C" fn on_state_changed(
    data: *mut c_void,
    _old: u32,
    state: u32,
    error: *const c_char,
) {
    let st = unsafe { &mut *(data as *mut State) };
    if state == PW_STREAM_STATE_ERROR {
        let msg = if error.is_null() {
            "stream error".to_string()
        } else {
            unsafe { std::ffi::CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        };
        *st.error.lock().unwrap() = Some(msg);
        unsafe { (st.pw.pw_main_loop_quit)(st.main_loop) };
    }
}

unsafe extern "C" fn on_param_changed(data: *mut c_void, id: u32, param: *const u8) {
    let st = unsafe { &mut *(data as *mut State) };
    if id != SPA_PARAM_FORMAT || param.is_null() {
        return;
    }
    if let Some(sz) = unsafe { Pod::format_size(param) } {
        *st.size.lock().unwrap() = Some(sz);
    }
    // Format agreed: now ask for cursor metadata on the buffers.
    let mut params: [*const u8; 1] = [st.meta_pod.as_ptr()];
    unsafe { (st.pw.pw_stream_update_params)(st.stream, params.as_mut_ptr(), 1) };
}

unsafe extern "C" fn on_process(data: *mut c_void) {
    let st = unsafe { &mut *(data as *mut State) };
    loop {
        let b = unsafe { (st.pw.pw_stream_dequeue_buffer)(st.stream) };
        if b.is_null() {
            break;
        }
        unsafe {
            let sb = (*b).buffer;
            if !sb.is_null() {
                let metas = std::slice::from_raw_parts((*sb).metas, (*sb).n_metas as usize);
                for m in metas {
                    if m.type_ == SPA_META_CURSOR
                        && m.size as usize >= SIZEOF_SPA_META_CURSOR
                        && !m.data.is_null()
                    {
                        let c = &*(m.data as *const SpaMetaCursor);
                        if c.id != 0 {
                            let _ = st.tx.send(Sample {
                                t_us: now_us(),
                                x: c.x,
                                y: c.y,
                            });
                        }
                    }
                }
            }
            (st.pw.pw_stream_queue_buffer)(st.stream, b);
        }
    }
}

/// A running cursor stream.  Drop or `stop()` to end it.
pub struct CursorStream {
    pub samples: Receiver<Sample>,
    size: Arc<Mutex<Option<(u32, u32)>>>,
    error: Arc<Mutex<Option<String>>>,
    quit: Box<dyn Fn() + Send>,
    thread: Option<JoinHandle<()>>,
}

impl CursorStream {
    /// Connect to the screen-cast node `node_id` over the portal-provided
    /// `fd`.  Returns once the stream thread is running.
    pub fn start(fd: RawFd, node_id: u32) -> Result<CursorStream, String> {
        let pw = Pw::load()?;
        let (tx, rx) = channel();
        let size = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = channel::<Result<(usize, usize), String>>();
        let (size2, error2) = (size.clone(), error.clone());

        let thread = std::thread::Builder::new()
            .name("atbswp-cursor-stream".into())
            .spawn(move || unsafe {
                (pw.pw_init)(std::ptr::null_mut(), std::ptr::null_mut());
                let main_loop = (pw.pw_main_loop_new)(std::ptr::null());
                let ctx = (pw.pw_context_new)(
                    (pw.pw_main_loop_get_loop)(main_loop),
                    std::ptr::null_mut(),
                    0,
                );
                let core = (pw.pw_context_connect_fd)(ctx, fd, std::ptr::null_mut(), 0);
                if core.is_null() {
                    let _ = ready_tx.send(Err("pw_context_connect_fd failed".into()));
                    return;
                }
                let props = (pw.pw_properties_new)(
                    c"media.type".as_ptr(),
                    c"Video".as_ptr(),
                    c"media.category".as_ptr(),
                    c"Capture".as_ptr(),
                    c"media.role".as_ptr(),
                    c"Screen".as_ptr(),
                    std::ptr::null::<c_char>(),
                );
                let stream = (pw.pw_stream_new)(core, c"atbswp cursor".as_ptr(), props);
                let state = Box::into_raw(Box::new(State {
                    pw,
                    stream,
                    main_loop,
                    tx,
                    size: size2,
                    error: error2,
                    meta_pod: Pod::meta_cursor(),
                }));
                let pw = &(*state).pw;
                let events = Box::new(PwStreamEvents {
                    version: 0,
                    destroy: None,
                    state_changed: Some(on_state_changed),
                    control_info: None,
                    io_changed: None,
                    param_changed: Some(on_param_changed),
                    add_buffer: None,
                    remove_buffer: None,
                    process: Some(on_process),
                    drained: None,
                    command: None,
                    trigger_done: None,
                });
                let mut hook = Box::new(SpaHook([0; SIZEOF_SPA_HOOK]));
                (pw.pw_stream_add_listener)(stream, &mut *hook, &*events, state as *mut c_void);
                let fmt = Pod::enum_format();
                let mut params: [*const u8; 1] = [fmt.as_ptr()];
                let rc = (pw.pw_stream_connect)(
                    stream,
                    PW_DIRECTION_INPUT,
                    node_id,
                    PW_STREAM_FLAG_AUTOCONNECT,
                    params.as_mut_ptr(),
                    1,
                );
                if rc < 0 {
                    let _ = ready_tx.send(Err(format!("pw_stream_connect failed ({rc})")));
                    return;
                }
                let _ = ready_tx.send(Ok((pw as *const Pw as usize, main_loop as usize)));
                (pw.pw_main_loop_run)(main_loop);
                (pw.pw_stream_destroy)(stream);
                (pw.pw_core_disconnect)(core);
                (pw.pw_context_destroy)(ctx);
                (pw.pw_main_loop_destroy)(main_loop);
                drop(Box::from_raw(state));
                drop(events);
                drop(hook);
            })
            .map_err(|e| e.to_string())?;

        let (pw_ptr, main_loop) = ready_rx
            .recv()
            .map_err(|_| "cursor stream thread died".to_string())??;
        let quit = Box::new(move || unsafe {
            // pw_main_loop_quit is safe to call from another thread.
            let pw = &*(pw_ptr as *const Pw);
            (pw.pw_main_loop_quit)(main_loop as *mut c_void);
        });
        Ok(CursorStream {
            samples: rx,
            size,
            error,
            quit,
            thread: Some(thread),
        })
    }

    /// Negotiated stream size in pixels, once the format is known.
    pub fn size(&self) -> Option<(u32, u32)> {
        *self.size.lock().unwrap()
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    pub fn stop(mut self) {
        (self.quit)();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_layout() {
        let id = Pod::id(2);
        assert_eq!(id, [4, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
        let f = Pod::enum_format();
        assert_eq!(
            u32::from_le_bytes(f[4..8].try_into().unwrap()),
            SPA_TYPE_OBJECT
        );
        assert_eq!(f.len() % 8, 0);
        assert_eq!(
            u32::from_le_bytes(f[0..4].try_into().unwrap()) as usize,
            f.len() - 8
        );
        // parse our own EnumFormat back: size is a Range choice with default 320x240
        assert_eq!(unsafe { Pod::format_size(f.as_ptr()) }, Some((320, 240)));
        let m = Pod::meta_cursor();
        assert_eq!(
            u32::from_le_bytes(m[8..12].try_into().unwrap()),
            SPA_TYPE_OBJECT_PARAM_META
        );
    }
}
