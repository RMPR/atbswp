/* Linux backend: libei via the XDG RemoteDesktop portal (Wayland), with an
 * XTest fallback for plain X11 sessions.
 *
 * Nothing here is linked at build time.  Every library is opened at runtime
 * with cosmo_dlopen() so that the same APE runs on a machine without libei
 * (it will fall back to X11) and does not need any of these libraries on
 * Windows or macOS.
 */
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/stat.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "player.h"

#ifdef __COSMOPOLITAN__
#include <dlfcn.h>
#define DLOPEN(p) cosmo_dlopen(p, RTLD_NOW)
#define DLSYM(h, n) cosmo_dltramp(cosmo_dlsym(h, n))
#define DLERROR() cosmo_dlerror()
#else
#include <dlfcn.h>
#define DLOPEN(p) dlopen(p, RTLD_NOW)
#define DLSYM(h, n) dlsym(h, n)
#define DLERROR() dlerror()
#endif

/* ====================================================================== */
/* libei                                                                  */
/* ====================================================================== */

struct ei;
struct ei_seat;
struct ei_device;
struct ei_event;
struct ei_region;

enum {
	EI_DEVICE_CAP_POINTER = 1 << 0,
	EI_DEVICE_CAP_POINTER_ABSOLUTE = 1 << 1,
	EI_DEVICE_CAP_KEYBOARD = 1 << 2,
	EI_DEVICE_CAP_SCROLL = 1 << 4,
	EI_DEVICE_CAP_BUTTON = 1 << 5,
};

enum {
	EI_EVENT_CONNECT = 1,
	EI_EVENT_DISCONNECT = 2,
	EI_EVENT_SEAT_ADDED = 3,
	EI_EVENT_SEAT_REMOVED = 4,
	EI_EVENT_DEVICE_ADDED = 5,
	EI_EVENT_DEVICE_REMOVED = 6,
	EI_EVENT_DEVICE_PAUSED = 7,
	EI_EVENT_DEVICE_RESUMED = 8,
};

#define EI_FUNCS(X) \
	X(struct ei *, ei_new_sender, (void *)) \
	X(struct ei *, ei_unref, (struct ei *)) \
	X(int, ei_setup_backend_fd, (struct ei *, int)) \
	X(int, ei_setup_backend_socket, (struct ei *, const char *)) \
	X(int, ei_get_fd, (struct ei *)) \
	X(void, ei_dispatch, (struct ei *)) \
	X(struct ei_event *, ei_get_event, (struct ei *)) \
	X(uint64_t, ei_now, (struct ei *)) \
	X(int, ei_event_get_type, (struct ei_event *)) \
	X(struct ei_seat *, ei_event_get_seat, (struct ei_event *)) \
	X(struct ei_device *, ei_event_get_device, (struct ei_event *)) \
	X(struct ei_event *, ei_event_unref, (struct ei_event *)) \
	X(void, ei_seat_bind_capabilities, (struct ei_seat *, ...)) \
	X(int, ei_device_has_capability, (struct ei_device *, int)) \
	X(struct ei_device *, ei_device_ref, (struct ei_device *)) \
	X(struct ei_device *, ei_device_unref, (struct ei_device *)) \
	X(struct ei_region *, ei_device_get_region, (struct ei_device *, size_t)) \
	X(uint32_t, ei_region_get_x, (struct ei_region *)) \
	X(uint32_t, ei_region_get_y, (struct ei_region *)) \
	X(uint32_t, ei_region_get_width, (struct ei_region *)) \
	X(uint32_t, ei_region_get_height, (struct ei_region *)) \
	X(void, ei_device_start_emulating, (struct ei_device *, uint32_t)) \
	X(void, ei_device_stop_emulating, (struct ei_device *)) \
	X(void, ei_device_frame, (struct ei_device *, uint64_t)) \
	X(void, ei_device_pointer_motion, (struct ei_device *, double, double)) \
	X(void, ei_device_pointer_motion_absolute, (struct ei_device *, double, double)) \
	X(void, ei_device_button_button, (struct ei_device *, uint32_t, bool)) \
	X(void, ei_device_scroll_discrete, (struct ei_device *, int32_t, int32_t)) \
	X(void, ei_device_keyboard_key, (struct ei_device *, uint32_t, bool))

#define DECL(ret, name, args) static ret (*p_##name) args;
EI_FUNCS(DECL)
#undef DECL

static bool load_ei(void)
{
	void *h = DLOPEN("libei.so.1");
	if (!h) {
		LOGV("libei.so.1 not available: %s\n", DLERROR());
		return false;
	}
#define LOAD(ret, name, args) \
	p_##name = (ret (*) args)DLSYM(h, #name); \
	if (!p_##name) { LOGE("libei: missing symbol %s\n", #name); return false; }
	EI_FUNCS(LOAD)
#undef LOAD
	return true;
}

#define MAX_DEVICES 16
struct ei_dev {
	struct ei_device *dev;
	int caps;
	bool resumed;
	bool emulating;
};

static struct {
	struct ei *ei;
	int fd;
	struct ei_dev devs[MAX_DEVICES];
	int ndevs;
	bool connected;
	bool disconnected;
	uint32_t sequence;
	/* absolute region bounding box */
	bool have_region;
	double rx, ry, rw, rh;
	/* accumulated hi-res scroll not yet emitted */
	int32_t scroll_acc_x, scroll_acc_y;
} E;

static void ei_handle_event(struct ei_event *ev)
{
	int type = p_ei_event_get_type(ev);
	switch (type) {
	case EI_EVENT_CONNECT:
		LOGV("ei: connected\n");
		E.connected = true;
		break;
	case EI_EVENT_DISCONNECT:
		LOGV("ei: disconnected by server\n");
		E.disconnected = true;
		break;
	case EI_EVENT_SEAT_ADDED: {
		struct ei_seat *seat = p_ei_event_get_seat(ev);
		LOGV("ei: seat added, binding capabilities\n");
		/* Variadic through a function pointer: fine on the SysV and
		 * AAPCS64 ABIs libei is ever built for. */
		p_ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_POINTER,
					    EI_DEVICE_CAP_POINTER_ABSOLUTE,
					    EI_DEVICE_CAP_KEYBOARD,
					    EI_DEVICE_CAP_BUTTON,
					    EI_DEVICE_CAP_SCROLL, (void *)0);
		break;
	}
	case EI_EVENT_DEVICE_ADDED: {
		struct ei_device *d = p_ei_event_get_device(ev);
		if (E.ndevs >= MAX_DEVICES)
			break;
		struct ei_dev *slot = &E.devs[E.ndevs++];
		slot->dev = p_ei_device_ref(d);
		slot->caps = 0;
		int all[] = { EI_DEVICE_CAP_POINTER, EI_DEVICE_CAP_POINTER_ABSOLUTE,
			      EI_DEVICE_CAP_KEYBOARD, EI_DEVICE_CAP_BUTTON, EI_DEVICE_CAP_SCROLL };
		for (size_t i = 0; i < sizeof(all) / sizeof(all[0]); i++)
			if (p_ei_device_has_capability(d, all[i]))
				slot->caps |= all[i];
		LOGV("ei: device added caps=0x%x\n", slot->caps);
		if (slot->caps & EI_DEVICE_CAP_POINTER_ABSOLUTE) {
			for (size_t i = 0;; i++) {
				struct ei_region *r = p_ei_device_get_region(d, i);
				if (!r)
					break;
				double x = p_ei_region_get_x(r), y = p_ei_region_get_y(r);
				double w = p_ei_region_get_width(r), h = p_ei_region_get_height(r);
				LOGV("ei:   region %g,%g %gx%g\n", x, y, w, h);
				if (!E.have_region) {
					E.rx = x; E.ry = y; E.rw = w; E.rh = h;
					E.have_region = true;
				} else {
					double x2 = E.rx + E.rw, y2 = E.ry + E.rh;
					if (x < E.rx) E.rx = x;
					if (y < E.ry) E.ry = y;
					if (x + w > x2) x2 = x + w;
					if (y + h > y2) y2 = y + h;
					E.rw = x2 - E.rx;
					E.rh = y2 - E.ry;
				}
			}
		}
		break;
	}
	case EI_EVENT_DEVICE_RESUMED:
	case EI_EVENT_DEVICE_PAUSED:
	case EI_EVENT_DEVICE_REMOVED: {
		struct ei_device *d = p_ei_event_get_device(ev);
		for (int i = 0; i < E.ndevs; i++) {
			if (E.devs[i].dev != d)
				continue;
			if (type == EI_EVENT_DEVICE_RESUMED) {
				E.devs[i].resumed = true;
			} else if (type == EI_EVENT_DEVICE_PAUSED) {
				E.devs[i].resumed = false;
				E.devs[i].emulating = false;
			} else {
				p_ei_device_unref(E.devs[i].dev);
				E.devs[i] = E.devs[--E.ndevs];
			}
			break;
		}
		LOGV("ei: device %s\n", type == EI_EVENT_DEVICE_RESUMED ? "resumed" :
		     type == EI_EVENT_DEVICE_PAUSED ? "paused" : "removed");
		break;
	}
	default:
		break;
	}
}

/* Service the libei fd for up to timeout_ms. Returns -1 on disconnect. */
static int ei_pump(int timeout_ms)
{
	struct pollfd pfd = { .fd = E.fd, .events = POLLIN };
	int r = poll(&pfd, 1, timeout_ms);
	if (r > 0) {
		p_ei_dispatch(E.ei);
		struct ei_event *ev;
		while ((ev = p_ei_get_event(E.ei))) {
			ei_handle_event(ev);
			p_ei_event_unref(ev);
		}
	}
	return E.disconnected ? -1 : 0;
}

static struct ei_dev *ei_find(int cap)
{
	for (int i = 0; i < E.ndevs; i++)
		if (E.devs[i].resumed && (E.devs[i].caps & cap))
			return &E.devs[i];
	return 0;
}

static struct ei_dev *ei_use(int cap)
{
	struct ei_dev *d = ei_find(cap);
	if (!d)
		return 0;
	if (!d->emulating) {
		p_ei_device_start_emulating(d->dev, ++E.sequence);
		d->emulating = true;
	}
	return d;
}

static void ei_frame(struct ei_dev *d)
{
	p_ei_device_frame(d->dev, p_ei_now(E.ei));
}

/* Wait until the server handed us usable devices. */
static int ei_wait_ready(void)
{
	uint64_t deadline = now_us() + 5000000;
	uint64_t settle = 0;
	while (now_us() < deadline) {
		if (ei_pump(50) != 0)
			return -1;
		bool have = ei_find(EI_DEVICE_CAP_KEYBOARD) ||
			    ei_find(EI_DEVICE_CAP_POINTER) ||
			    ei_find(EI_DEVICE_CAP_POINTER_ABSOLUTE);
		if (have) {
			if (!settle)
				settle = now_us() + 200000;	/* let the rest arrive */
			else if (now_us() >= settle)
				return 0;
		}
	}
	if (!E.connected)
		LOGE("ei: server never accepted the connection\n");
	else
		LOGE("ei: no input devices offered within 5s\n");
	return -1;
}

/* ====================================================================== */
/* XDG RemoteDesktop portal over libdbus                                  */
/* ====================================================================== */

typedef struct { unsigned char opaque[128] __attribute__((aligned(8))); } DBusMessageIter;
typedef struct {
	const char *name;
	const char *message;
	unsigned char opaque[64];
} DBusError;
struct DBusConnection;
struct DBusMessage;

enum {
	DBUS_BUS_SESSION = 0,
	DBUS_TYPE_INVALID = 0,
	DBUS_TYPE_ARRAY = 'a',
	DBUS_TYPE_DICT_ENTRY = 'e',
	DBUS_TYPE_UNIX_FD = 'h',
	DBUS_TYPE_OBJECT_PATH = 'o',
	DBUS_TYPE_STRING = 's',
	DBUS_TYPE_UINT32 = 'u',
	DBUS_TYPE_VARIANT = 'v',
};

#define DBUS_FUNCS(X) \
	X(void, dbus_error_init, (DBusError *)) \
	X(int, dbus_error_is_set, (const DBusError *)) \
	X(void, dbus_error_free, (DBusError *)) \
	X(struct DBusConnection *, dbus_bus_get, (int, DBusError *)) \
	X(const char *, dbus_bus_get_unique_name, (struct DBusConnection *)) \
	X(void, dbus_bus_add_match, (struct DBusConnection *, const char *, DBusError *)) \
	X(void, dbus_connection_flush, (struct DBusConnection *)) \
	X(int, dbus_connection_read_write, (struct DBusConnection *, int)) \
	X(struct DBusMessage *, dbus_connection_pop_message, (struct DBusConnection *)) \
	X(struct DBusMessage *, dbus_connection_send_with_reply_and_block, (struct DBusConnection *, struct DBusMessage *, int, DBusError *)) \
	X(struct DBusMessage *, dbus_message_new_method_call, (const char *, const char *, const char *, const char *)) \
	X(void, dbus_message_unref, (struct DBusMessage *)) \
	X(int, dbus_message_is_signal, (struct DBusMessage *, const char *, const char *)) \
	X(const char *, dbus_message_get_path, (struct DBusMessage *)) \
	X(void, dbus_message_iter_init_append, (struct DBusMessage *, DBusMessageIter *)) \
	X(int, dbus_message_iter_append_basic, (DBusMessageIter *, int, const void *)) \
	X(int, dbus_message_iter_open_container, (DBusMessageIter *, int, const char *, DBusMessageIter *)) \
	X(int, dbus_message_iter_close_container, (DBusMessageIter *, DBusMessageIter *)) \
	X(int, dbus_message_iter_init, (struct DBusMessage *, DBusMessageIter *)) \
	X(int, dbus_message_iter_get_arg_type, (DBusMessageIter *)) \
	X(void, dbus_message_iter_get_basic, (DBusMessageIter *, void *)) \
	X(void, dbus_message_iter_recurse, (DBusMessageIter *, DBusMessageIter *)) \
	X(int, dbus_message_iter_next, (DBusMessageIter *))

#define DECL(ret, name, args) static ret (*p_##name) args;
DBUS_FUNCS(DECL)
#undef DECL

static bool load_dbus(void)
{
	void *h = DLOPEN("libdbus-1.so.3");
	if (!h) {
		LOGV("libdbus-1.so.3 not available: %s\n", DLERROR());
		return false;
	}
#define LOAD(ret, name, args) \
	p_##name = (ret (*) args)DLSYM(h, #name); \
	if (!p_##name) { LOGE("libdbus: missing symbol %s\n", #name); return false; }
	DBUS_FUNCS(LOAD)
#undef LOAD
	return true;
}

#define PORTAL_BUS "org.freedesktop.portal.Desktop"
#define PORTAL_PATH "/org/freedesktop/portal/desktop"
#define PORTAL_RD "org.freedesktop.portal.RemoteDesktop"
#define PORTAL_REQUEST "org.freedesktop.portal.Request"

struct portal {
	struct DBusConnection *conn;
	char sender[256];	/* unique name mangled for request paths */
	unsigned token_counter;
	char session[512];
	char restore_token[512];
	char token_path[1024];
};

/* Append one {sv} entry with a string or uint32 value to an open a{sv}. */
static void dict_add_str(DBusMessageIter *dict, const char *key, const char *val, int type)
{
	DBusMessageIter entry, var;
	char sig[2] = { (char)type, 0 };
	p_dbus_message_iter_open_container(dict, DBUS_TYPE_DICT_ENTRY, 0, &entry);
	p_dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &key);
	p_dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, sig, &var);
	p_dbus_message_iter_append_basic(&var, type, &val);
	p_dbus_message_iter_close_container(&entry, &var);
	p_dbus_message_iter_close_container(dict, &entry);
}

static void dict_add_u32(DBusMessageIter *dict, const char *key, uint32_t val)
{
	DBusMessageIter entry, var;
	p_dbus_message_iter_open_container(dict, DBUS_TYPE_DICT_ENTRY, 0, &entry);
	p_dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &key);
	p_dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "u", &var);
	p_dbus_message_iter_append_basic(&var, DBUS_TYPE_UINT32, &val);
	p_dbus_message_iter_close_container(&entry, &var);
	p_dbus_message_iter_close_container(dict, &entry);
}

/* Parse a Response(u, a{sv}) signal; copy interesting string results. */
static uint32_t parse_response(struct DBusMessage *msg, struct portal *P)
{
	DBusMessageIter it, dict;
	uint32_t response = 99;
	if (!p_dbus_message_iter_init(msg, &it))
		return response;
	if (p_dbus_message_iter_get_arg_type(&it) != DBUS_TYPE_UINT32)
		return response;
	p_dbus_message_iter_get_basic(&it, &response);
	if (!p_dbus_message_iter_next(&it) || p_dbus_message_iter_get_arg_type(&it) != DBUS_TYPE_ARRAY)
		return response;
	p_dbus_message_iter_recurse(&it, &dict);
	while (p_dbus_message_iter_get_arg_type(&dict) == DBUS_TYPE_DICT_ENTRY) {
		DBusMessageIter entry, var;
		const char *key = 0;
		p_dbus_message_iter_recurse(&dict, &entry);
		if (p_dbus_message_iter_get_arg_type(&entry) == DBUS_TYPE_STRING) {
			p_dbus_message_iter_get_basic(&entry, &key);
			if (p_dbus_message_iter_next(&entry) &&
			    p_dbus_message_iter_get_arg_type(&entry) == DBUS_TYPE_VARIANT) {
				p_dbus_message_iter_recurse(&entry, &var);
				int t = p_dbus_message_iter_get_arg_type(&var);
				if (t == DBUS_TYPE_STRING || t == DBUS_TYPE_OBJECT_PATH) {
					const char *s = 0;
					p_dbus_message_iter_get_basic(&var, &s);
					if (s && !strcmp(key, "session_handle"))
						snprintf(P->session, sizeof(P->session), "%s", s);
					else if (s && !strcmp(key, "restore_token"))
						snprintf(P->restore_token, sizeof(P->restore_token), "%s", s);
				}
			}
		}
		p_dbus_message_iter_next(&dict);
	}
	return response;
}

static void fill_create_session(DBusMessageIter *args, DBusMessageIter *opts, struct portal *P)
{
	(void)args;
	char st[64];
	snprintf(st, sizeof(st), "atbswp_s%u", (unsigned)getpid());
	dict_add_str(opts, "session_handle_token", st, DBUS_TYPE_STRING);
	(void)P;
}

static void fill_select_devices(DBusMessageIter *args, DBusMessageIter *opts, struct portal *P)
{
	(void)args;
	dict_add_u32(opts, "types", 1 | 2);	/* keyboard | pointer */
	dict_add_u32(opts, "persist_mode", 2);	/* persist until revoked */
	if (P->restore_token[0])
		dict_add_str(opts, "restore_token", P->restore_token, DBUS_TYPE_STRING);
}

/* Call a RemoteDesktop method whose result arrives as a Request.Response
 * signal.  Arguments are ([o session,] [s parent_window,] a{sv} options);
 * `fill` may append further option entries.  Returns the portal response
 * code (0 = success, 1 = user cancelled, 2 = other), or -1 on error. */
static int portal_request(struct portal *P, const char *method, bool with_session, bool with_parent,
			  void (*fill)(DBusMessageIter *, DBusMessageIter *, struct portal *),
			  int timeout_ms)
{
	char token[64], request_path[600];
	snprintf(token, sizeof(token), "atbswp%u_%u", (unsigned)getpid(), ++P->token_counter);
	snprintf(request_path, sizeof(request_path), "/org/freedesktop/portal/desktop/request/%s/%s",
		 P->sender, token);
	char rule[800];
	snprintf(rule, sizeof(rule),
		 "type='signal',interface='" PORTAL_REQUEST "',member='Response',path='%s'", request_path);
	DBusError err;
	p_dbus_error_init(&err);
	p_dbus_bus_add_match(P->conn, rule, &err);
	if (p_dbus_error_is_set(&err)) {
		LOGE("portal: add_match failed: %s\n", err.message);
		p_dbus_error_free(&err);
		return -1;
	}
	struct DBusMessage *msg = p_dbus_message_new_method_call(PORTAL_BUS, PORTAL_PATH, PORTAL_RD, method);
	DBusMessageIter args, opts;
	p_dbus_message_iter_init_append(msg, &args);
	const char *session = P->session;
	if (with_session)
		p_dbus_message_iter_append_basic(&args, DBUS_TYPE_OBJECT_PATH, &session);
	if (with_parent) {
		const char *parent = "";
		p_dbus_message_iter_append_basic(&args, DBUS_TYPE_STRING, &parent);
	}
	p_dbus_message_iter_open_container(&args, DBUS_TYPE_ARRAY, "{sv}", &opts);
	dict_add_str(&opts, "handle_token", token, DBUS_TYPE_STRING);
	if (fill)
		fill(&args, &opts, P);
	p_dbus_message_iter_close_container(&args, &opts);

	struct DBusMessage *reply = p_dbus_connection_send_with_reply_and_block(P->conn, msg, 5000, &err);
	p_dbus_message_unref(msg);
	if (!reply) {
		LOGE("portal: %s failed: %s\n", method, err.message ? err.message : "unknown error");
		p_dbus_error_free(&err);
		return -1;
	}
	char alt_path[600] = "";
	DBusMessageIter rit;
	if (p_dbus_message_iter_init(reply, &rit) &&
	    p_dbus_message_iter_get_arg_type(&rit) == DBUS_TYPE_OBJECT_PATH) {
		const char *h = 0;
		p_dbus_message_iter_get_basic(&rit, &h);
		if (h && strcmp(h, request_path)) {
			snprintf(alt_path, sizeof(alt_path), "%s", h);
			snprintf(rule, sizeof(rule),
				 "type='signal',interface='" PORTAL_REQUEST "',member='Response',path='%s'", alt_path);
			p_dbus_bus_add_match(P->conn, rule, 0);
		}
	}
	p_dbus_message_unref(reply);

	uint64_t deadline = now_us() + (uint64_t)timeout_ms * 1000;
	while (now_us() < deadline) {
		if (!p_dbus_connection_read_write(P->conn, 100)) {
			LOGE("portal: bus connection closed\n");
			return -1;
		}
		struct DBusMessage *m;
		while ((m = p_dbus_connection_pop_message(P->conn))) {
			int rc = -2;
			if (p_dbus_message_is_signal(m, PORTAL_REQUEST, "Response")) {
				const char *path = p_dbus_message_get_path(m);
				if (path && (!strcmp(path, request_path) || (*alt_path && !strcmp(path, alt_path))))
					rc = (int)parse_response(m, P);
			}
			p_dbus_message_unref(m);
			if (rc != -2)
				return rc;
		}
	}
	LOGE("portal: timed out waiting for %s response\n", method);
	return -1;
}

static void token_file_path(char *buf, size_t n)
{
	const char *state = getenv("XDG_STATE_HOME");
	const char *home = getenv("HOME");
	if (state && *state)
		snprintf(buf, n, "%s/atbswp-portal-token", state);
	else if (home && *home)
		snprintf(buf, n, "%s/.local/state/atbswp-portal-token", home);
	else
		buf[0] = 0;
}

static void load_restore_token(struct portal *P)
{
	token_file_path(P->token_path, sizeof(P->token_path));
	if (!P->token_path[0])
		return;
	/* The token grants input access: ignore it unless it is a regular file
	 * we own that nobody else can read. */
	int fd = open(P->token_path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
	if (fd < 0)
		return;
	struct stat st;
	if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != getuid() || (st.st_mode & 077)) {
		LOGV("portal: ignoring restore token %s (wrong owner or permissions)\n", P->token_path);
		close(fd);
		return;
	}
	ssize_t n = read(fd, P->restore_token, sizeof(P->restore_token) - 1);
	close(fd);
	if (n <= 0) {
		P->restore_token[0] = 0;
		return;
	}
	P->restore_token[n] = 0;
	char *nl = strpbrk(P->restore_token, "\r\n");
	if (nl)
		*nl = 0;
}

static void save_restore_token(struct portal *P)
{
	if (!P->token_path[0] || !P->restore_token[0])
		return;
	/* best effort: create the state directory (up to two levels), private */
	char dir[1024];
	snprintf(dir, sizeof(dir), "%s", P->token_path);
	char *slash = strrchr(dir, '/');
	if (slash) {
		*slash = 0;
		char *parent = strrchr(dir, '/');
		if (parent && parent != dir) {
			*parent = 0;
			mkdir(dir, 0700);
			*parent = '/';
		}
		mkdir(dir, 0700);
	}
	/* write 0600 to a private temp name, then rename atomically; never
	 * follow a symlink someone may have planted at the final path */
	char tmp[1100];
	snprintf(tmp, sizeof(tmp), "%s.%lld.tmp", P->token_path, (long long)getpid());
	int fd = open(tmp, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
	if (fd < 0) {
		LOGV("portal: cannot save restore token to %s: %s\n", tmp, strerror(errno));
		return;
	}
	size_t len = strlen(P->restore_token);
	bool ok = write(fd, P->restore_token, len) == (ssize_t)len && write(fd, "\n", 1) == 1;
	ok = close(fd) == 0 && ok;
	struct stat st;
	bool planted = lstat(P->token_path, &st) == 0 && S_ISLNK(st.st_mode);
	if (!ok || planted || rename(tmp, P->token_path) != 0) {
		LOGV("portal: cannot save restore token to %s\n", P->token_path);
		unlink(tmp);
	}
}

/* Returns an EIS fd, or -1. */
static int portal_connect_eis(void)
{
	if (!load_dbus())
		return -1;
	static struct portal P;
	memset(&P, 0, sizeof(P));
	DBusError err;
	p_dbus_error_init(&err);
	P.conn = p_dbus_bus_get(DBUS_BUS_SESSION, &err);
	if (!P.conn) {
		LOGE("portal: cannot connect to session bus: %s\n", err.message ? err.message : "?");
		p_dbus_error_free(&err);
		return -1;
	}
	const char *unique = p_dbus_bus_get_unique_name(P.conn);
	if (!unique || unique[0] != ':') {
		LOGE("portal: bad unique name\n");
		return -1;
	}
	snprintf(P.sender, sizeof(P.sender), "%s", unique + 1);
	for (char *c = P.sender; *c; c++)
		if (*c == '.')
			*c = '_';
	load_restore_token(&P);

	int rc = portal_request(&P, "CreateSession", false, false, fill_create_session, 10000);
	if (rc != 0 || !P.session[0]) {
		LOGE("portal: CreateSession failed (%d)\n", rc);
		return -1;
	}
	LOGV("portal: session %s\n", P.session);
	rc = portal_request(&P, "SelectDevices", true, false, fill_select_devices, 10000);
	if (rc != 0) {
		LOGE("portal: SelectDevices failed (%d)\n", rc);
		return -1;
	}
	P.restore_token[0] = 0;
	rc = portal_request(&P, "Start", true, true, 0, 300000);
	if (rc != 0) {
		LOGE("portal: Start failed or was denied (%d)\n", rc);
		return -1;
	}
	save_restore_token(&P);

	struct DBusMessage *msg = p_dbus_message_new_method_call(PORTAL_BUS, PORTAL_PATH, PORTAL_RD, "ConnectToEIS");
	DBusMessageIter args, opts;
	p_dbus_message_iter_init_append(msg, &args);
	const char *session = P.session;
	p_dbus_message_iter_append_basic(&args, DBUS_TYPE_OBJECT_PATH, &session);
	p_dbus_message_iter_open_container(&args, DBUS_TYPE_ARRAY, "{sv}", &opts);
	p_dbus_message_iter_close_container(&args, &opts);
	struct DBusMessage *reply = p_dbus_connection_send_with_reply_and_block(P.conn, msg, 5000, &err);
	p_dbus_message_unref(msg);
	if (!reply) {
		LOGE("portal: ConnectToEIS failed: %s\n", err.message ? err.message : "?");
		p_dbus_error_free(&err);
		return -1;
	}
	int fd = -1;
	DBusMessageIter rit;
	if (p_dbus_message_iter_init(reply, &rit) && p_dbus_message_iter_get_arg_type(&rit) == DBUS_TYPE_UNIX_FD)
		p_dbus_message_iter_get_basic(&rit, &fd);
	p_dbus_message_unref(reply);
	if (fd < 0)
		LOGE("portal: ConnectToEIS returned no fd\n");
	return fd;
}

/* ====================================================================== */
/* libei backend glue                                                     */
/* ====================================================================== */

static int ei_connect(void)
{
	if (!load_ei())
		return -1;
	E.ei = p_ei_new_sender(0);
	if (!E.ei)
		return -1;
	int rc;
	if (getenv("LIBEI_SOCKET")) {
		LOGV("ei: using LIBEI_SOCKET=%s\n", getenv("LIBEI_SOCKET"));
		rc = p_ei_setup_backend_socket(E.ei, 0);
		if (rc) {
			LOGE("ei: socket backend failed: %s\n", strerror(-rc));
			return -1;
		}
	} else {
		int fd = portal_connect_eis();
		if (fd < 0)
			return -1;
		rc = p_ei_setup_backend_fd(E.ei, fd);
		if (rc) {
			LOGE("ei: fd backend failed: %s\n", strerror(-rc));
			return -1;
		}
	}
	E.fd = p_ei_get_fd(E.ei);
	return ei_wait_ready();
}

/* ====================================================================== */
/* XTest fallback                                                         */
/* ====================================================================== */

struct Display;
#define X_FUNCS(X) \
	X(struct Display *, XOpenDisplay, (const char *)) \
	X(int, XCloseDisplay, (struct Display *)) \
	X(int, XDefaultScreen, (struct Display *)) \
	X(int, XDisplayWidth, (struct Display *, int)) \
	X(int, XDisplayHeight, (struct Display *, int)) \
	X(int, XFlush, (struct Display *))
#define XT_FUNCS(X) \
	X(int, XTestFakeMotionEvent, (struct Display *, int, int, int, unsigned long)) \
	X(int, XTestFakeRelativeMotionEvent, (struct Display *, int, int, unsigned long)) \
	X(int, XTestFakeButtonEvent, (struct Display *, unsigned, int, unsigned long)) \
	X(int, XTestFakeKeyEvent, (struct Display *, unsigned, int, unsigned long))
#define DECL(ret, name, args) static ret (*p_##name) args;
X_FUNCS(DECL)
XT_FUNCS(DECL)
#undef DECL

static struct Display *xdpy;
static int xscreen;

static int xtest_connect(uint32_t *w, uint32_t *h)
{
	if (!getenv("DISPLAY"))
		return -1;
	void *hx = DLOPEN("libX11.so.6");
	void *ht = DLOPEN("libXtst.so.6");
	if (!hx || !ht) {
		LOGV("X11/XTest libraries not available\n");
		return -1;
	}
#define LOAD(ret, name, args) \
	p_##name = (ret (*) args)DLSYM(name##_handle, #name); \
	if (!p_##name) { LOGE("x11: missing symbol %s\n", #name); return -1; }
#define XOpenDisplay_handle hx
#define XCloseDisplay_handle hx
#define XDefaultScreen_handle hx
#define XDisplayWidth_handle hx
#define XDisplayHeight_handle hx
#define XFlush_handle hx
#define XTestFakeMotionEvent_handle ht
#define XTestFakeRelativeMotionEvent_handle ht
#define XTestFakeButtonEvent_handle ht
#define XTestFakeKeyEvent_handle ht
	X_FUNCS(LOAD)
	XT_FUNCS(LOAD)
#undef LOAD
	xdpy = p_XOpenDisplay(0);
	if (!xdpy) {
		LOGE("x11: cannot open display\n");
		return -1;
	}
	xscreen = p_XDefaultScreen(xdpy);
	*w = (uint32_t)p_XDisplayWidth(xdpy, xscreen);
	*h = (uint32_t)p_XDisplayHeight(xdpy, xscreen);
	return 0;
}

static unsigned xtest_button(uint16_t code)
{
	switch (code) {
	case ATBSWP_BTN_LEFT: return 1;
	case ATBSWP_BTN_MIDDLE: return 2;
	case ATBSWP_BTN_RIGHT: return 3;
	case ATBSWP_BTN_SIDE: return 8;
	case ATBSWP_BTN_EXTRA: return 9;
	default: return 0;
	}
}

/* ====================================================================== */
/* injector                                                               */
/* ====================================================================== */

static enum { MODE_NONE, MODE_EI, MODE_XTEST } mode;

static int lx_init(uint32_t *w, uint32_t *h)
{
	*w = *h = 0;
	const char *force = getenv("ATBSWP_BACKEND");
	bool try_ei = !force || !strcmp(force, "ei");
	bool try_x = !force || !strcmp(force, "xtest");
	if (try_ei && ei_connect() == 0) {
		mode = MODE_EI;
		if (E.have_region) {
			*w = (uint32_t)E.rw;
			*h = (uint32_t)E.rh;
		}
		return 0;
	}
	if (try_x && xtest_connect(w, h) == 0) {
		mode = MODE_XTEST;
		LOGV("using XTest fallback\n");
		return 0;
	}
	LOGE("no usable input backend (need a Wayland session with the RemoteDesktop portal, or X11)\n");
	return -1;
}

static void lx_move_abs(int32_t x, int32_t y)
{
	if (mode == MODE_EI) {
		struct ei_dev *d = ei_use(EI_DEVICE_CAP_POINTER_ABSOLUTE);
		if (!d) {
			LOGV("ei: no absolute pointer device\n");
			return;
		}
		p_ei_device_pointer_motion_absolute(d->dev, E.rx + x, E.ry + y);
		ei_frame(d);
	} else if (mode == MODE_XTEST) {
		p_XTestFakeMotionEvent(xdpy, xscreen, x, y, 0);
		p_XFlush(xdpy);
	}
}

static void lx_move_rel(int32_t dx, int32_t dy)
{
	if (mode == MODE_EI) {
		struct ei_dev *d = ei_use(EI_DEVICE_CAP_POINTER);
		if (!d)
			return;
		p_ei_device_pointer_motion(d->dev, dx, dy);
		ei_frame(d);
	} else if (mode == MODE_XTEST) {
		p_XTestFakeRelativeMotionEvent(xdpy, dx, dy, 0);
		p_XFlush(xdpy);
	}
}

static void lx_button(uint16_t code, bool pressed)
{
	if (mode == MODE_EI) {
		struct ei_dev *d = ei_use(EI_DEVICE_CAP_BUTTON);
		if (!d)
			return;
		p_ei_device_button_button(d->dev, code, pressed);
		ei_frame(d);
	} else if (mode == MODE_XTEST) {
		unsigned b = xtest_button(code);
		if (b) {
			p_XTestFakeButtonEvent(xdpy, b, pressed, 0);
			p_XFlush(xdpy);
		}
	}
}

static void lx_key(uint16_t code, bool pressed)
{
	if (mode == MODE_EI) {
		struct ei_dev *d = ei_use(EI_DEVICE_CAP_KEYBOARD);
		if (!d)
			return;
		p_ei_device_keyboard_key(d->dev, code, pressed);
		ei_frame(d);
	} else if (mode == MODE_XTEST) {
		p_XTestFakeKeyEvent(xdpy, code + 8u, pressed, 0);	/* X keycode = evdev + 8 */
		p_XFlush(xdpy);
	}
}

static void lx_scroll(int32_t x, int32_t y)
{
	if (mode == MODE_EI) {
		struct ei_dev *d = ei_use(EI_DEVICE_CAP_SCROLL);
		if (!d)
			return;
		p_ei_device_scroll_discrete(d->dev, x, y);
		ei_frame(d);
	} else if (mode == MODE_XTEST) {
		/* X has only whole notches: accumulate and emit clicks. */
		E.scroll_acc_x += x;
		E.scroll_acc_y += y;
		while (E.scroll_acc_y >= 120) { p_XTestFakeButtonEvent(xdpy, 5, 1, 0); p_XTestFakeButtonEvent(xdpy, 5, 0, 0); E.scroll_acc_y -= 120; }
		while (E.scroll_acc_y <= -120) { p_XTestFakeButtonEvent(xdpy, 4, 1, 0); p_XTestFakeButtonEvent(xdpy, 4, 0, 0); E.scroll_acc_y += 120; }
		while (E.scroll_acc_x >= 120) { p_XTestFakeButtonEvent(xdpy, 7, 1, 0); p_XTestFakeButtonEvent(xdpy, 7, 0, 0); E.scroll_acc_x -= 120; }
		while (E.scroll_acc_x <= -120) { p_XTestFakeButtonEvent(xdpy, 6, 1, 0); p_XTestFakeButtonEvent(xdpy, 6, 0, 0); E.scroll_acc_x += 120; }
		p_XFlush(xdpy);
	}
}

static int lx_idle(uint32_t us)
{
	if (mode == MODE_EI)
		return ei_pump((int)(us / 1000));
	sleep_us(us);
	return 0;
}

static void lx_shutdown(void)
{
	if (mode == MODE_EI) {
		for (int i = 0; i < E.ndevs; i++) {
			if (E.devs[i].emulating)
				p_ei_device_stop_emulating(E.devs[i].dev);
			p_ei_device_unref(E.devs[i].dev);
		}
		E.ndevs = 0;
		/* Give the server a moment to drain before we go away. */
		ei_pump(50);
		p_ei_unref(E.ei);
		E.ei = 0;
	} else if (mode == MODE_XTEST) {
		p_XFlush(xdpy);
		p_XCloseDisplay(xdpy);
	}
}

const struct injector injector_linux = {
	.name = "linux",
	.init = lx_init,
	.move_abs = lx_move_abs,
	.move_rel = lx_move_rel,
	.button = lx_button,
	.key = lx_key,
	.scroll = lx_scroll,
	.idle = lx_idle,
	.shutdown = lx_shutdown,
};
