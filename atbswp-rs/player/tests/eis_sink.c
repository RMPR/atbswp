/* Minimal EIS server for tests: accepts one libei sender client, offers a
 * pointer, an absolute pointer (1920x1080 region) and a keyboard, and logs
 * every event it receives, one per line, to stdout.
 *
 * usage: eis_sink SOCKET_PATH [--timeout SECONDS]
 * Exits 0 when the client disconnects, 3 on timeout.
 */
#include <errno.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "libeis.h"

static double now(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ts.tv_sec + ts.tv_nsec / 1e9;
}

static struct eis_device *add_device(struct eis_seat *seat, const char *name, int caps, bool abs)
{
	struct eis_device *d = eis_seat_new_device(seat);
	eis_device_configure_type(d, EIS_DEVICE_TYPE_VIRTUAL);
	eis_device_configure_name(d, name);
	for (int c = 1; c <= EIS_DEVICE_CAP_BUTTON; c <<= 1)
		if (caps & c)
			eis_device_configure_capability(d, c);
	if (abs) {
		struct eis_region *r = eis_device_new_region(d);
		eis_region_set_size(r, 1920, 1080);
		eis_region_set_offset(r, 0, 0);
		eis_region_add(r);
		eis_region_unref(r);
	}
	eis_device_add(d);
	eis_device_resume(d);
	return d;
}

int main(int argc, char **argv)
{
	if (argc < 2) {
		fprintf(stderr, "usage: %s SOCKET_PATH [--timeout S]\n", argv[0]);
		return 64;
	}
	double timeout = 30;
	if (argc >= 4 && !strcmp(argv[2], "--timeout"))
		timeout = atof(argv[3]);

	struct eis *eis = eis_new(NULL);
	unlink(argv[1]);
	if (eis_setup_backend_socket(eis, argv[1]) != 0) {
		perror("eis_setup_backend_socket");
		return 1;
	}
	setvbuf(stdout, NULL, _IOLBF, 0);
	printf("listening %s\n", argv[1]);

	double deadline = now() + timeout;
	struct pollfd pfd = { .fd = eis_get_fd(eis), .events = POLLIN };
	for (;;) {
		if (now() > deadline) {
			printf("timeout\n");
			return 3;
		}
		if (poll(&pfd, 1, 100) <= 0)
			continue;
		eis_dispatch(eis);
		struct eis_event *e;
		while ((e = eis_get_event(eis))) {
			switch (eis_event_get_type(e)) {
			case EIS_EVENT_CLIENT_CONNECT: {
				struct eis_client *c = eis_event_get_client(e);
				printf("client connect sender=%d name=%s\n", eis_client_is_sender(c),
				       eis_client_get_name(c) ? eis_client_get_name(c) : "?");
				eis_client_connect(c);
				struct eis_seat *seat = eis_client_new_seat(c, "default");
				eis_seat_configure_capability(seat, EIS_DEVICE_CAP_POINTER);
				eis_seat_configure_capability(seat, EIS_DEVICE_CAP_POINTER_ABSOLUTE);
				eis_seat_configure_capability(seat, EIS_DEVICE_CAP_KEYBOARD);
				eis_seat_configure_capability(seat, EIS_DEVICE_CAP_BUTTON);
				eis_seat_configure_capability(seat, EIS_DEVICE_CAP_SCROLL);
				eis_seat_add(seat);
				eis_seat_unref(seat);
				break;
			}
			case EIS_EVENT_CLIENT_DISCONNECT:
				printf("client disconnect\n");
				eis_event_unref(e);
				return 0;
			case EIS_EVENT_SEAT_BIND: {
				struct eis_seat *seat = eis_event_get_seat(e);
				static bool devices_added;
				bool any = eis_event_seat_has_capability(e, EIS_DEVICE_CAP_POINTER) ||
					   eis_event_seat_has_capability(e, EIS_DEVICE_CAP_KEYBOARD) ||
					   eis_event_seat_has_capability(e, EIS_DEVICE_CAP_POINTER_ABSOLUTE);
				printf("seat bind%s\n", any ? "" : " (unbind)");
				if (!any || devices_added)
					break;
				devices_added = true;
				add_device(seat, "test pointer", EIS_DEVICE_CAP_POINTER | EIS_DEVICE_CAP_BUTTON | EIS_DEVICE_CAP_SCROLL, false);
				add_device(seat, "test abs pointer", EIS_DEVICE_CAP_POINTER_ABSOLUTE | EIS_DEVICE_CAP_BUTTON | EIS_DEVICE_CAP_SCROLL, true);
				add_device(seat, "test keyboard", EIS_DEVICE_CAP_KEYBOARD, false);
				break;
			}
			case EIS_EVENT_DEVICE_START_EMULATING:
				printf("start_emulating %s\n", eis_device_get_name(eis_event_get_device(e)));
				break;
			case EIS_EVENT_DEVICE_STOP_EMULATING:
				printf("stop_emulating %s\n", eis_device_get_name(eis_event_get_device(e)));
				break;
			case EIS_EVENT_FRAME:
				printf("frame\n");
				break;
			case EIS_EVENT_POINTER_MOTION:
				printf("motion %g %g\n", eis_event_pointer_get_dx(e), eis_event_pointer_get_dy(e));
				break;
			case EIS_EVENT_POINTER_MOTION_ABSOLUTE:
				printf("absolute %g %g\n", eis_event_pointer_get_absolute_x(e), eis_event_pointer_get_absolute_y(e));
				break;
			case EIS_EVENT_BUTTON_BUTTON:
				printf("button %u %d\n", eis_event_button_get_button(e), eis_event_button_get_is_press(e));
				break;
			case EIS_EVENT_KEYBOARD_KEY:
				printf("key %u %d\n", eis_event_keyboard_get_key(e), eis_event_keyboard_get_key_is_press(e));
				break;
			case EIS_EVENT_SCROLL_DISCRETE:
				printf("scroll_discrete %d %d\n", eis_event_scroll_get_discrete_dx(e), eis_event_scroll_get_discrete_dy(e));
				break;
			case EIS_EVENT_SCROLL_DELTA:
				printf("scroll %g %g\n", eis_event_scroll_get_dx(e), eis_event_scroll_get_dy(e));
				break;
			case EIS_EVENT_DEVICE_CLOSED:
				printf("device closed %s\n", eis_device_get_name(eis_event_get_device(e)));
				break;
			default:
				printf("event %d\n", eis_event_get_type(e));
			}
			eis_event_unref(e);
		}
	}
}
