/* Prints the SPA/PipeWire ABI facts crates/atbswp-core/src/record/pw/abi.rs
 * hard-codes, so CI can diff them against the real headers. */
#include <stdio.h>
#include <stddef.h>
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/buffer/meta.h>
#define P(rust, c) printf("%s %u\n", rust, (unsigned)(c))
int main(void)
{
	P("SPA_META_CURSOR", SPA_META_Cursor);
	P("SPA_TYPE_ID", SPA_TYPE_Id); P("SPA_TYPE_INT", SPA_TYPE_Int);
	P("SPA_TYPE_RECTANGLE", SPA_TYPE_Rectangle); P("SPA_TYPE_FRACTION", SPA_TYPE_Fraction);
	P("SPA_TYPE_OBJECT", SPA_TYPE_Object); P("SPA_TYPE_CHOICE", SPA_TYPE_Choice);
	P("SPA_CHOICE_RANGE", SPA_CHOICE_Range); P("SPA_CHOICE_ENUM", SPA_CHOICE_Enum);
	P("SPA_TYPE_OBJECT_FORMAT", SPA_TYPE_OBJECT_Format); P("SPA_TYPE_OBJECT_PARAM_META", SPA_TYPE_OBJECT_ParamMeta);
	P("SPA_PARAM_ENUM_FORMAT", SPA_PARAM_EnumFormat); P("SPA_PARAM_FORMAT", SPA_PARAM_Format); P("SPA_PARAM_META", SPA_PARAM_Meta);
	P("SPA_PARAM_META_TYPE", SPA_PARAM_META_type); P("SPA_PARAM_META_SIZE", SPA_PARAM_META_size);
	P("SPA_FORMAT_MEDIA_TYPE", SPA_FORMAT_mediaType); P("SPA_FORMAT_MEDIA_SUBTYPE", SPA_FORMAT_mediaSubtype);
	P("SPA_FORMAT_VIDEO_FORMAT", SPA_FORMAT_VIDEO_format); P("SPA_FORMAT_VIDEO_SIZE", SPA_FORMAT_VIDEO_size);
	P("SPA_FORMAT_VIDEO_FRAMERATE", SPA_FORMAT_VIDEO_framerate);
	P("SPA_MEDIA_TYPE_VIDEO", SPA_MEDIA_TYPE_video); P("SPA_MEDIA_SUBTYPE_RAW", SPA_MEDIA_SUBTYPE_raw);
	P("SPA_VIDEO_FORMAT_RGBX", SPA_VIDEO_FORMAT_RGBx); P("SPA_VIDEO_FORMAT_BGRX", SPA_VIDEO_FORMAT_BGRx);
	P("SPA_VIDEO_FORMAT_RGBA", SPA_VIDEO_FORMAT_RGBA); P("SPA_VIDEO_FORMAT_BGRA", SPA_VIDEO_FORMAT_BGRA);
	P("SPA_VIDEO_FORMAT_RGB", SPA_VIDEO_FORMAT_RGB); P("SPA_VIDEO_FORMAT_BGR", SPA_VIDEO_FORMAT_BGR);
	P("PW_DIRECTION_INPUT", PW_DIRECTION_INPUT); P("PW_STREAM_FLAG_AUTOCONNECT", PW_STREAM_FLAG_AUTOCONNECT);
	P("PW_STREAM_STATE_ERROR", PW_STREAM_STATE_ERROR);
	P("SIZEOF_SPA_HOOK", sizeof(struct spa_hook));
	P("SIZEOF_SPA_META_CURSOR", sizeof(struct spa_meta_cursor)); P("SIZEOF_SPA_META_BITMAP", sizeof(struct spa_meta_bitmap));
	/* layouts the Rust structs mirror */
	P("offsetof_pw_stream_events_param_changed", offsetof(struct pw_stream_events, param_changed));
	P("offsetof_pw_stream_events_process", offsetof(struct pw_stream_events, process));
	P("offsetof_spa_buffer_metas", offsetof(struct spa_buffer, metas));
	P("offsetof_spa_meta_data", offsetof(struct spa_meta, data));
	P("offsetof_spa_meta_cursor_position", offsetof(struct spa_meta_cursor, position));
	P("sizeof_pw_buffer", sizeof(struct pw_buffer));
	return 0;
}
