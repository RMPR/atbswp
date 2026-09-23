//! Serialising and reading SPA "PODs", PipeWire's self-describing binary
//! values: the video format we accept, the cursor-metadata request, and the
//! negotiated size read back from a Format object.

use super::abi::*;

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

#[cfg(test)]
mod tests {
    use super::super::abi::*;
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
