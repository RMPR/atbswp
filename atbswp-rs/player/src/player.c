/* atbswp standalone macro player.
 *
 * This program is compiled once with cosmocc into an Actually Portable
 * Executable.  The recorder appends a macro payload (see macro_format.h) to
 * a copy of it; at startup we read our own file, find the footer, and replay
 * the events through the backend for the OS we happen to be running on.
 */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "player.h"

#ifdef __COSMOPOLITAN__
#include <libc/dce.h>
#include <libc/runtime/runtime.h>
#elif defined(__APPLE__)
/* Native macOS build: the x86-64 helper bundled into the APE for Intel Macs
 * (cosmopolitan cannot dlopen there), see delegate_to_native_helper(). */
#include <mach-o/dyld.h>
#define IsWindows() 0
#define IsXnu() 1
#define IsLinux() 0
static char *GetProgramExecutableName(void)
{
	static char buf[4096];
	uint32_t n = sizeof(buf);
	if (_NSGetExecutablePath(buf, &n) != 0)
		return 0;
	return buf;
}
#else
/* Native Linux build, used for quick host-only testing. */
#define IsWindows() 0
#define IsXnu() 0
#define IsLinux() 1
#include <unistd.h>
static char *GetProgramExecutableName(void)
{
	static char buf[4096];
	ssize_t n = readlink("/proc/self/exe", buf, sizeof(buf) - 1);
	if (n < 0)
		return 0;
	buf[n] = 0;
	return buf;
}
#endif

#ifdef __COSMOPOLITAN__
#include <sys/stat.h>
#include <unistd.h>
#include <fcntl.h>

#define HELPER_MAGIC "ATBSWPH1"

/* Intel Macs: cosmopolitan cannot dlopen CoreGraphics there, so a native
 * x86-64 Mach-O build of this very program can be bundled right behind the
 * APE, ahead of the macro payload:
 *
 *   player.com | helper | u64 helper_len | "ATBSWPH1" | payload | u64 | "ATBSWPM1"
 *
 * (-mtiny drops cosmopolitan's /zip/ filesystem, hence our own footer.)
 * Returns the helper's offset and length in `path`, or -1 if none. */
static int find_helper(const char *path, uint64_t *off, uint64_t *len)
{
	FILE *f = fopen(path, "rb");
	if (!f)
		return -1;
	int rc = -1;
	if (fseek(f, 0, SEEK_END) == 0) {
		long end = ftell(f);
		uint8_t footer[ATBSWP_FOOTER_LEN];
		/* skip the macro payload footer if present */
		if (end >= ATBSWP_FOOTER_LEN && fseek(f, end - ATBSWP_FOOTER_LEN, SEEK_SET) == 0 &&
		    fread(footer, 1, sizeof(footer), f) == sizeof(footer) &&
		    !memcmp(footer + 8, ATBSWP_MAGIC, 8)) {
			uint64_t plen = 0;
			for (int i = 7; i >= 0; i--)
				plen = (plen << 8) | footer[i];
			end -= (long)(ATBSWP_FOOTER_LEN + plen);
		}
		if (end >= ATBSWP_FOOTER_LEN && fseek(f, end - ATBSWP_FOOTER_LEN, SEEK_SET) == 0 &&
		    fread(footer, 1, sizeof(footer), f) == sizeof(footer) &&
		    !memcmp(footer + 8, HELPER_MAGIC, 8)) {
			uint64_t hlen = 0;
			for (int i = 7; i >= 0; i--)
				hlen = (hlen << 8) | footer[i];
			if (hlen && hlen <= (uint64_t)end - ATBSWP_FOOTER_LEN) {
				*len = hlen;
				*off = (uint64_t)end - ATBSWP_FOOTER_LEN - hlen;
				rc = 0;
			}
		}
	}
	fclose(f);
	return rc;
}

/* Extract the bundled helper once to $TMPDIR and exec it with our payload. */
static int delegate_to_native_helper(const char *self, const char *payload, int argc, char **argv)
{
	uint64_t off, len;
	if (find_helper(self, &off, &len) != 0) {
		LOGE("Intel macOS needs the bundled native helper, which this player lacks\n");
		return -1;
	}
	const char *tmp = getenv("TMPDIR");
	if (!tmp || !*tmp)
		tmp = "/tmp";
	char dst[4096];
	snprintf(dst, sizeof(dst), "%s/atbswp-helper-%llu-%llu", tmp, (unsigned long long)len,
		 (unsigned long long)off);
	struct stat cur;
	if (stat(dst, &cur) != 0 || (uint64_t)cur.st_size != len) {
		FILE *in = fopen(self, "rb");
		int out = open(dst, O_WRONLY | O_CREAT | O_TRUNC, 0755);
		if (!in || out < 0 || fseek(in, (long)off, SEEK_SET) != 0) {
			LOGE("cannot extract helper to %s: %s\n", dst, strerror(errno));
			return -1;
		}
		char buf[65536];
		uint64_t left = len;
		while (left) {
			size_t want = left < sizeof(buf) ? (size_t)left : sizeof(buf);
			size_t n = fread(buf, 1, want, in);
			if (!n || write(out, buf, n) != (ssize_t)n) {
				LOGE("short copy extracting helper\n");
				fclose(in);
				close(out);
				unlink(dst);
				return -1;
			}
			left -= n;
		}
		fclose(in);
		close(out);
		chmod(dst, 0755);
	}
	LOGV("delegating to native helper %s\n", dst);
	char **nargv = calloc((size_t)argc + 4, sizeof(char *));
	int k = 0;
	nargv[k++] = dst;
	nargv[k++] = "--payload";
	nargv[k++] = (char *)payload;
	for (int i = 1; i < argc; i++) {
		if (!strcmp(argv[i], "--payload") && i + 1 < argc) {
			i++;
			continue;
		}
		nargv[k++] = argv[i];
	}
	nargv[k] = 0;
	execv(dst, nargv);
	LOGE("exec %s: %s\n", dst, strerror(errno));
	return -1;
}
#endif

#define ATBSWP_PLAYER_VERSION "0.1.0"

int verbose;

uint64_t now_us(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (uint64_t)ts.tv_sec * 1000000u + ts.tv_nsec / 1000;
}

void sleep_us(uint64_t us)
{
	struct timespec ts = { us / 1000000u, (us % 1000000u) * 1000 };
	while (nanosleep(&ts, &ts) == -1 && errno == EINTR)
		;
}

/* ---- payload loading ------------------------------------------------- */

struct macro {
	atbswp_header hdr;
	atbswp_event *events;
	size_t n;
};

static int parse_payload(const uint8_t *buf, size_t len, struct macro *m)
{
	if (len < sizeof(atbswp_header)) {
		LOGE("payload too short (%zu bytes)\n", len);
		return -1;
	}
	memcpy(&m->hdr, buf, sizeof(m->hdr));
	if (m->hdr.version != ATBSWP_FORMAT_VERSION) {
		LOGE("unsupported payload version %u\n", m->hdr.version);
		return -1;
	}
	size_t want = sizeof(atbswp_header) + (size_t)m->hdr.event_count * sizeof(atbswp_event);
	if (want != len) {
		LOGE("payload size mismatch: header says %zu, have %zu\n", want, len);
		return -1;
	}
	m->n = m->hdr.event_count;
	m->events = malloc(m->n * sizeof(atbswp_event) + 1);
	if (!m->events)
		return -1;
	memcpy(m->events, buf + sizeof(atbswp_header), m->n * sizeof(atbswp_event));
	return 0;
}

static int read_file(const char *path, uint8_t **out, size_t *len)
{
	FILE *f = fopen(path, "rb");
	if (!f) {
		LOGE("cannot open %s: %s\n", path, strerror(errno));
		return -1;
	}
	if (fseek(f, 0, SEEK_END) != 0) {
		fclose(f);
		return -1;
	}
	long sz = ftell(f);
	if (sz < 0) {
		fclose(f);
		return -1;
	}
	rewind(f);
	uint8_t *buf = malloc((size_t)sz + 1);
	if (!buf) {
		fclose(f);
		return -1;
	}
	if (fread(buf, 1, (size_t)sz, f) != (size_t)sz) {
		LOGE("short read on %s\n", path);
		free(buf);
		fclose(f);
		return -1;
	}
	fclose(f);
	*out = buf;
	*len = (size_t)sz;
	return 0;
}

/* Locate the payload in an executable (or any file carrying our footer). */
static int load_from_executable(const char *path, struct macro *m)
{
	FILE *f = fopen(path, "rb");
	if (!f) {
		LOGE("cannot open %s: %s\n", path, strerror(errno));
		return -1;
	}
	uint8_t footer[ATBSWP_FOOTER_LEN];
	if (fseek(f, -ATBSWP_FOOTER_LEN, SEEK_END) != 0 ||
	    fread(footer, 1, sizeof(footer), f) != sizeof(footer)) {
		LOGE("%s: cannot read footer\n", path);
		fclose(f);
		return -1;
	}
	if (memcmp(footer + 8, ATBSWP_MAGIC, ATBSWP_MAGIC_LEN) != 0) {
		LOGE("%s: no macro payload found (this is a bare player)\n", path);
		fclose(f);
		return -1;
	}
	uint64_t plen = 0;
	for (int i = 7; i >= 0; i--)
		plen = (plen << 8) | footer[i];
	if (plen > (64u << 20)) {
		LOGE("%s: implausible payload length\n", path);
		fclose(f);
		return -1;
	}
	if (fseek(f, -(long)(ATBSWP_FOOTER_LEN + plen), SEEK_END) != 0) {
		fclose(f);
		return -1;
	}
	uint8_t *buf = malloc((size_t)plen + 1);
	if (!buf || fread(buf, 1, (size_t)plen, f) != plen) {
		LOGE("%s: short read on payload\n", path);
		free(buf);
		fclose(f);
		return -1;
	}
	fclose(f);
	int rc = parse_payload(buf, (size_t)plen, m);
	free(buf);
	return rc;
}

/* ---- dumping --------------------------------------------------------- */

static const char *type_name(uint16_t t)
{
	switch (t) {
	case ATBSWP_EV_MOVE_ABS: return "move";
	case ATBSWP_EV_MOVE_REL: return "moverel";
	case ATBSWP_EV_BUTTON_PRESS: return "buttondown";
	case ATBSWP_EV_BUTTON_RELEASE: return "buttonup";
	case ATBSWP_EV_KEY_PRESS: return "keydown";
	case ATBSWP_EV_KEY_RELEASE: return "keyup";
	case ATBSWP_EV_SCROLL: return "scroll";
	default: return "unknown";
	}
}

static void dump(const struct macro *m)
{
	printf("# atbswp macro v%u: %zu events, screen %ux%u, repeat %u, speed %u%%\n",
	       m->hdr.version, m->n, m->hdr.screen_w, m->hdr.screen_h,
	       m->hdr.repeat, m->hdr.speed_percent);
	for (size_t i = 0; i < m->n; i++) {
		const atbswp_event *e = &m->events[i];
		if (e->delay_us)
			printf("wait %uus\n", e->delay_us);
		switch (e->type) {
		case ATBSWP_EV_MOVE_ABS:
		case ATBSWP_EV_MOVE_REL:
		case ATBSWP_EV_SCROLL:
			printf("%s %d %d\n", type_name(e->type), e->x, e->y);
			break;
		default:
			printf("%s %u\n", type_name(e->type), e->code);
		}
	}
}

/* ---- playback -------------------------------------------------------- */

struct play_opts {
	uint32_t repeat;
	uint32_t speed_percent;
	uint32_t start_delay_ms;
};

static const struct injector *pick_injector(bool dry_run)
{
	if (dry_run)
		return &injector_dry_run;
	if (IsWindows())
		return &injector_windows;
	if (IsXnu())
		return &injector_macos;
	return &injector_linux;
}

static int play(const struct macro *m, const struct injector *inj, const struct play_opts *o)
{
	uint32_t sw = 0, sh = 0;
	if (inj->init(&sw, &sh) != 0) {
		LOGE("%s backend failed to initialise\n", inj->name);
		return 1;
	}
	LOGV("backend %s ready, target screen %ux%u, recorded %ux%u\n", inj->name, sw, sh,
	     m->hdr.screen_w, m->hdr.screen_h);
	bool scale = sw && sh && m->hdr.screen_w && m->hdr.screen_h &&
		     (sw != m->hdr.screen_w || sh != m->hdr.screen_h);
	if (o->start_delay_ms)
		sleep_us((uint64_t)o->start_delay_ms * 1000);

	int rc = 0;
	uint32_t speed = o->speed_percent ? o->speed_percent : 100;
	for (uint32_t iter = 0; o->repeat == 0 || iter < o->repeat; iter++) {
		uint64_t t0 = now_us();
		uint64_t cursor = 0;	/* virtual time in recorded microseconds */
		for (size_t i = 0; i < m->n && rc == 0; i++) {
			const atbswp_event *e = &m->events[i];
			cursor += e->delay_us;
			uint64_t target = t0 + cursor * 100 / speed;
			for (;;) {
				uint64_t now = now_us();
				if (now >= target)
					break;
				uint64_t left = target - now;
				if (inj->idle) {
					if (inj->idle((uint32_t)(left > 50000 ? 50000 : left)) != 0) {
						rc = 2;
						break;
					}
				} else {
					sleep_us(left);
				}
			}
			if (rc)
				break;
			switch (e->type) {
			case ATBSWP_EV_MOVE_ABS: {
				int32_t x = e->x, y = e->y;
				if (scale) {
					x = (int32_t)((int64_t)x * sw / m->hdr.screen_w);
					y = (int32_t)((int64_t)y * sh / m->hdr.screen_h);
				}
				inj->move_abs(x, y);
				break;
			}
			case ATBSWP_EV_MOVE_REL:
				if (e->x || e->y)	/* a zero move only carries a delay */
					inj->move_rel(e->x, e->y);
				break;
			case ATBSWP_EV_BUTTON_PRESS:
			case ATBSWP_EV_BUTTON_RELEASE:
				inj->button(e->code, e->type == ATBSWP_EV_BUTTON_PRESS);
				break;
			case ATBSWP_EV_KEY_PRESS:
			case ATBSWP_EV_KEY_RELEASE:
				inj->key(e->code, e->type == ATBSWP_EV_KEY_PRESS);
				break;
			case ATBSWP_EV_SCROLL:
				inj->scroll(e->x, e->y);
				break;
			default:
				LOGV("skipping unknown event type %u\n", e->type);
			}
		}
		if (rc)
			break;
	}
	inj->shutdown();
	return rc;
}

/* ---- main ------------------------------------------------------------ */

static void usage(const char *argv0)
{
	printf("usage: %s [options]\n"
	       "\n"
	       "Replays the macro embedded in this executable.\n"
	       "\n"
	       "  --repeat N        play N times (0 = forever; default from macro)\n"
	       "  --speed PCT       playback speed in percent (default from macro)\n"
	       "  --start-delay MS  wait before the first event\n"
	       "  --dump            print the macro instead of playing it\n"
	       "  --dry-run         print events with real timing, inject nothing\n"
	       "  --payload FILE    read the payload from FILE instead of this executable\n"
	       "  --verbose         chatty diagnostics on stderr\n"
	       "  --version, --help\n", argv0);
}

int main(int argc, char **argv)
{
	bool do_dump = false, dry_run = false;
	const char *payload_path = 0;
	struct play_opts o = { 0 };
	bool have_repeat = false, have_speed = false;

	for (int i = 1; i < argc; i++) {
		const char *a = argv[i];
		if (!strcmp(a, "--help") || !strcmp(a, "-h")) {
			usage(argv[0]);
			return 0;
		} else if (!strcmp(a, "--version")) {
			printf("atbswp player %s\n", ATBSWP_PLAYER_VERSION);
			return 0;
		} else if (!strcmp(a, "--dump")) {
			do_dump = true;
		} else if (!strcmp(a, "--dry-run")) {
			dry_run = true;
		} else if (!strcmp(a, "--verbose") || !strcmp(a, "-v")) {
			verbose = 1;
		} else if (!strcmp(a, "--payload") && i + 1 < argc) {
			payload_path = argv[++i];
		} else if (!strcmp(a, "--repeat") && i + 1 < argc) {
			o.repeat = (uint32_t)strtoul(argv[++i], 0, 10);
			have_repeat = true;
		} else if (!strcmp(a, "--speed") && i + 1 < argc) {
			o.speed_percent = (uint32_t)strtoul(argv[++i], 0, 10);
			have_speed = true;
		} else if (!strcmp(a, "--start-delay") && i + 1 < argc) {
			o.start_delay_ms = (uint32_t)strtoul(argv[++i], 0, 10);
		} else {
			LOGE("unknown argument: %s\n", a);
			usage(argv[0]);
			return 64;
		}
	}
	if (getenv("ATBSWP_VERBOSE"))
		verbose = 1;

	struct macro m = { 0 };
	if (payload_path) {
		uint8_t *buf;
		size_t len;
		if (read_file(payload_path, &buf, &len) != 0)
			return 1;
		/* Accept either a raw payload or a file with a footer. */
		int rc;
		if (len >= ATBSWP_FOOTER_LEN &&
		    !memcmp(buf + len - ATBSWP_MAGIC_LEN, ATBSWP_MAGIC, ATBSWP_MAGIC_LEN))
			rc = load_from_executable(payload_path, &m);
		else
			rc = parse_payload(buf, len, &m);
		free(buf);
		if (rc)
			return 1;
	} else {
		const char *self = GetProgramExecutableName();
		if (!self) {
			LOGE("cannot determine own executable path\n");
			return 1;
		}
		if (load_from_executable(self, &m) != 0)
			return 1;
	}

	if (do_dump) {
		dump(&m);
		free(m.events);
		return 0;
	}
#ifdef __COSMOPOLITAN__
	{
		const char *self = GetProgramExecutableName();
		uint64_t hoff, hlen;
		if (self && find_helper(self, &hoff, &hlen) == 0)
			LOGV("bundled Intel macOS helper present (%llu bytes)\n", (unsigned long long)hlen);
		if (!dry_run && IsXnu() && !IsXnuSilicon()) {
			if (delegate_to_native_helper(self, payload_path ? payload_path : self, argc, argv) != 0) {
				free(m.events);
				return 1;
			}
		}
	}
#endif
	if (!have_repeat)
		o.repeat = m.hdr.repeat; /* 0 = forever */
	if (!have_speed)
		o.speed_percent = m.hdr.speed_percent ? m.hdr.speed_percent : 100;

	int rc = play(&m, pick_injector(dry_run), &o);
	free(m.events);
	return rc;
}
