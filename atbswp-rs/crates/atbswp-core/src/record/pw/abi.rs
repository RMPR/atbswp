//! The stable SPA/PipeWire ABI the recorder relies on: enum values, struct
//! layouts and the libpipewire entry points, all declared by hand so that
//! no PipeWire headers or libraries are needed at build time.  CI checks
//! every value here against the real headers with `tests/pw_abi_check.c`.

use super::super::dl::dl_api;
use std::ffi::{c_char, c_int, c_void};

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
pub struct SpaHook(pub [u8; SIZEOF_SPA_HOOK]);

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
