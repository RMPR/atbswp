//! Cursor positions from a PipeWire screen-cast stream, using only the
//! `spa_meta_cursor` metadata attached to each buffer (pixels are never
//! mapped).  `libpipewire-0.3.so.0` is loaded at run time; the constants and
//! struct layouts below are the stable SPA/PipeWire ABI, checked against the
//! real headers by `tests/pw_abi_check.c` in CI.

mod abi;
mod pod;

use abi::*;
use pod::Pod;
use std::ffi::{c_char, c_void};
use std::os::unix::io::RawFd;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

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
                                t_us: super::monotonic_us(),
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
