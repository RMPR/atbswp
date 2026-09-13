/* Windows backend: user32!SendInput loaded at runtime.
 *
 * Function pointers use the Microsoft x64 calling convention explicitly so
 * no runtime ABI trampoline is needed.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "player.h"

#if defined(__COSMOPOLITAN__) && defined(__x86_64__)
#include <dlfcn.h>
#define HAVE_WIN 1
#define MSABI __attribute__((__ms_abi__))
#endif

#ifdef HAVE_WIN

typedef struct {
	int32_t dx, dy;
	uint32_t mouseData, dwFlags, time;
	uintptr_t dwExtraInfo;
} MOUSEINPUT_;

typedef struct {
	uint16_t wVk, wScan;
	uint32_t dwFlags, time;
	uintptr_t dwExtraInfo;
} KEYBDINPUT_;

typedef struct {
	uint32_t type;
	union {
		MOUSEINPUT_ mi;
		KEYBDINPUT_ ki;
	};
} INPUT_;

_Static_assert(sizeof(INPUT_) == 40, "INPUT layout");

enum {
	INPUT_MOUSE = 0,
	INPUT_KEYBOARD = 1,
	MOUSEEVENTF_MOVE = 0x0001,
	MOUSEEVENTF_LEFTDOWN = 0x0002,
	MOUSEEVENTF_LEFTUP = 0x0004,
	MOUSEEVENTF_RIGHTDOWN = 0x0008,
	MOUSEEVENTF_RIGHTUP = 0x0010,
	MOUSEEVENTF_MIDDLEDOWN = 0x0020,
	MOUSEEVENTF_MIDDLEUP = 0x0040,
	MOUSEEVENTF_XDOWN = 0x0080,
	MOUSEEVENTF_XUP = 0x0100,
	MOUSEEVENTF_WHEEL = 0x0800,
	MOUSEEVENTF_HWHEEL = 0x1000,
	MOUSEEVENTF_VIRTUALDESK = 0x4000,
	MOUSEEVENTF_ABSOLUTE = 0x8000,
	KEYEVENTF_EXTENDEDKEY = 0x0001,
	KEYEVENTF_KEYUP = 0x0002,
	KEYEVENTF_SCANCODE = 0x0008,
	SM_XVIRTUALSCREEN = 76,
	SM_YVIRTUALSCREEN = 77,
	SM_CXVIRTUALSCREEN = 78,
	SM_CYVIRTUALSCREEN = 79,
	XBUTTON1 = 1,
	XBUTTON2 = 2,
};

static uint32_t (MSABI *pSendInput)(uint32_t, INPUT_ *, int);
static int (MSABI *pGetSystemMetrics)(int);

static int32_t vx, vy, vw, vh;

static int win_init(uint32_t *w, uint32_t *h)
{
	void *u32 = cosmo_dlopen("user32.dll", RTLD_NOW);
	if (!u32) {
		LOGE("cannot load user32.dll: %s\n", cosmo_dlerror());
		return -1;
	}
	pSendInput = (uint32_t (MSABI *)(uint32_t, INPUT_ *, int))cosmo_dlsym(u32, "SendInput");
	pGetSystemMetrics = (int (MSABI *)(int))cosmo_dlsym(u32, "GetSystemMetrics");
	if (!pSendInput || !pGetSystemMetrics) {
		LOGE("user32.dll is missing SendInput/GetSystemMetrics\n");
		return -1;
	}
	vx = pGetSystemMetrics(SM_XVIRTUALSCREEN);
	vy = pGetSystemMetrics(SM_YVIRTUALSCREEN);
	vw = pGetSystemMetrics(SM_CXVIRTUALSCREEN);
	vh = pGetSystemMetrics(SM_CYVIRTUALSCREEN);
	if (vw <= 0 || vh <= 0) {
		LOGE("GetSystemMetrics reported no screen\n");
		return -1;
	}
	*w = (uint32_t)vw;
	*h = (uint32_t)vh;
	return 0;
}

static void send_mouse(int32_t dx, int32_t dy, uint32_t data, uint32_t flags)
{
	INPUT_ in;
	memset(&in, 0, sizeof(in));
	in.type = INPUT_MOUSE;
	in.mi.dx = dx;
	in.mi.dy = dy;
	in.mi.mouseData = data;
	in.mi.dwFlags = flags;
	if (pSendInput(1, &in, sizeof(in)) != 1)
		LOGV("SendInput(mouse) failed\n");
}

static void win_move_abs(int32_t x, int32_t y)
{
	/* Normalise to 0..65535 across the virtual desktop. */
	int64_t nx = ((int64_t)(x - vx) * 65535 + (vw - 1) / 2) / (vw - 1 ? vw - 1 : 1);
	int64_t ny = ((int64_t)(y - vy) * 65535 + (vh - 1) / 2) / (vh - 1 ? vh - 1 : 1);
	if (nx < 0) nx = 0;
	if (ny < 0) ny = 0;
	if (nx > 65535) nx = 65535;
	if (ny > 65535) ny = 65535;
	send_mouse((int32_t)nx, (int32_t)ny, 0, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK);
}

static void win_move_rel(int32_t dx, int32_t dy)
{
	send_mouse(dx, dy, 0, MOUSEEVENTF_MOVE);
}

static void win_button(uint16_t code, bool pressed)
{
	uint32_t flags = 0, data = 0;
	switch (code) {
	case ATBSWP_BTN_LEFT: flags = pressed ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP; break;
	case ATBSWP_BTN_RIGHT: flags = pressed ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_RIGHTUP; break;
	case ATBSWP_BTN_MIDDLE: flags = pressed ? MOUSEEVENTF_MIDDLEDOWN : MOUSEEVENTF_MIDDLEUP; break;
	case ATBSWP_BTN_SIDE: flags = pressed ? MOUSEEVENTF_XDOWN : MOUSEEVENTF_XUP; data = XBUTTON1; break;
	case ATBSWP_BTN_EXTRA: flags = pressed ? MOUSEEVENTF_XDOWN : MOUSEEVENTF_XUP; data = XBUTTON2; break;
	default:
		LOGV("unmapped button %u\n", code);
		return;
	}
	send_mouse(0, 0, data, flags);
}

static void win_key(uint16_t code, bool pressed)
{
	uint16_t sc;
	bool ext;
	if (!keymap_evdev_to_win(code, &sc, &ext)) {
		LOGV("unmapped key %u\n", code);
		return;
	}
	INPUT_ in;
	memset(&in, 0, sizeof(in));
	in.type = INPUT_KEYBOARD;
	in.ki.wScan = sc;
	in.ki.dwFlags = KEYEVENTF_SCANCODE | (ext ? KEYEVENTF_EXTENDEDKEY : 0) | (pressed ? 0 : KEYEVENTF_KEYUP);
	if (pSendInput(1, &in, sizeof(in)) != 1)
		LOGV("SendInput(key) failed\n");
}

static void win_scroll(int32_t x, int32_t y)
{
	/* Our unit (1/120 notch) equals WHEEL_DELTA/120, so pass through.
	 * Windows: positive = away from user (up); ours: positive = down. */
	if (y)
		send_mouse(0, 0, (uint32_t)(-y), MOUSEEVENTF_WHEEL);
	if (x)
		send_mouse(0, 0, (uint32_t)x, MOUSEEVENTF_HWHEEL);
}

static void win_shutdown(void) {}

const struct injector injector_windows = {
	.name = "windows",
	.init = win_init,
	.move_abs = win_move_abs,
	.move_rel = win_move_rel,
	.button = win_button,
	.key = win_key,
	.scroll = win_scroll,
	.idle = 0,
	.shutdown = win_shutdown,
};

#else /* !HAVE_WIN */

static int win_init(uint32_t *w, uint32_t *h)
{
	(void)w; (void)h;
	LOGE("Windows backend not compiled into this build\n");
	return -1;
}
static void win_nop2(int32_t a, int32_t b) { (void)a; (void)b; }
static void win_nopb(uint16_t a, bool b) { (void)a; (void)b; }
static void win_nop(void) {}
const struct injector injector_windows = {
	.name = "windows", .init = win_init, .move_abs = win_nop2, .move_rel = win_nop2,
	.button = win_nopb, .key = win_nopb, .scroll = win_nop2, .idle = 0, .shutdown = win_nop,
};

#endif
