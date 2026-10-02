/* Test stand-in for a compositor's screen-cast node: a PipeWire video
 * source that attaches spa_meta_cursor to every buffer and moves the
 * cursor along a scripted path.  Prints "node <id>" once registered.
 *
 *   pw_cursor_src X1,Y1 X2,Y2 ...    (one position per frame, 20 fps, loops)
 *
 * Built against libpipewire-0.3-dev; only used by tests/e2e_wayland_record.sh.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/buffer/meta.h>

#define CURSOR_META_SIZE(w, h) (sizeof(struct spa_meta_cursor) + sizeof(struct spa_meta_bitmap) + (w) * (h) * 4)
#define W 1024
#define H 768

struct data {
	struct pw_main_loop *loop;
	struct pw_stream *stream;
	struct spa_source *timer;
	int npos;
	int (*pos)[2];
	int frame;
	int announced;
};

static void on_process(void *userdata)
{
	struct data *d = userdata;
	struct pw_buffer *b = pw_stream_dequeue_buffer(d->stream);
	if (!b)
		return;
	struct spa_buffer *buf = b->buffer;
	struct spa_meta_cursor *mc = spa_buffer_find_meta_data(buf, SPA_META_Cursor, sizeof(*mc));
	if (mc) {
		int i = d->frame < d->npos ? d->frame : d->npos - 1; /* hold the last position */
		mc->id = 1;
		mc->flags = 0;
		mc->position.x = d->pos[i][0];
		mc->position.y = d->pos[i][1];
		mc->hotspot.x = 0;
		mc->hotspot.y = 0;
		mc->bitmap_offset = 0;
	}
	if (buf->datas[0].data) {
		buf->datas[0].chunk->offset = 0;
		buf->datas[0].chunk->stride = W * 4;
		buf->datas[0].chunk->size = W * H * 4;
	}
	d->frame++;
	pw_stream_queue_buffer(d->stream, b);
}

static void on_timer(void *userdata, uint64_t expirations)
{
	struct data *d = userdata;
	(void)expirations;
	if (!d->announced && pw_stream_get_node_id(d->stream) != SPA_ID_INVALID) {
		printf("node %u\n", pw_stream_get_node_id(d->stream));
		fflush(stdout);
		d->announced = 1;
	}
	pw_stream_trigger_process(d->stream);
}

static void on_param_changed(void *userdata, uint32_t id, const struct spa_pod *param)
{
	struct data *d = userdata;
	if (id != SPA_PARAM_Format || !param)
		return;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[2];
	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
		SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(4, 2, 8),
		SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1),
		SPA_PARAM_BUFFERS_size, SPA_POD_Int(W * H * 4),
		SPA_PARAM_BUFFERS_stride, SPA_POD_Int(W * 4));
	params[1] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
		SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Cursor),
		SPA_PARAM_META_size, SPA_POD_Int(CURSOR_META_SIZE(64, 64)));
	pw_stream_update_params(d->stream, params, 2);
}

static const struct pw_stream_events events = {
	PW_VERSION_STREAM_EVENTS,
	.process = on_process,
	.param_changed = on_param_changed,
};

int main(int argc, char **argv)
{
	if (argc < 2) {
		fprintf(stderr, "usage: %s X,Y [X,Y...]\n", argv[0]);
		return 64;
	}
	struct data d = { 0 };
	d.npos = argc - 1;
	d.pos = calloc(d.npos, sizeof(*d.pos));
	for (int i = 0; i < d.npos; i++)
		sscanf(argv[i + 1], "%d,%d", &d.pos[i][0], &d.pos[i][1]);

	pw_init(&argc, &argv);
	d.loop = pw_main_loop_new(NULL);
	d.stream = pw_stream_new_simple(pw_main_loop_get_loop(d.loop), "fake-screencast",
		pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture",
				  PW_KEY_MEDIA_ROLE, "Screen", PW_KEY_NODE_NAME, "atbswp-fake-screencast", NULL),
		&events, &d);

	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	params[0] = spa_format_video_raw_build(&b, SPA_PARAM_EnumFormat,
		&SPA_VIDEO_INFO_RAW_INIT(.format = SPA_VIDEO_FORMAT_BGRx,
					 .size = SPA_RECTANGLE(W, H),
					 .framerate = SPA_FRACTION(20, 1)));
	if (pw_stream_connect(d.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			      PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 1) < 0) {
		fprintf(stderr, "pw_stream_connect failed\n");
		return 1;
	}
	struct pw_loop *loop = pw_main_loop_get_loop(d.loop);
	d.timer = pw_loop_add_timer(loop, on_timer, &d);
	struct timespec iv = { 0, 50 * 1000 * 1000 };
	pw_loop_update_timer(loop, d.timer, &iv, &iv, false);
	pw_main_loop_run(d.loop);
	return 0;
}
