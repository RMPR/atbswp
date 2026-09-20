/* macOS backend: CoreGraphics event taps, loaded at runtime.
 *
 * Cosmopolitan can only dlopen on Apple Silicon; on Intel Macs the player
 * reports an error.  The binary needs Accessibility permission (System
 * Settings > Privacy & Security > Accessibility) to post events.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "player.h"

#ifdef __COSMOPOLITAN__
#include <dlfcn.h>
#include <libc/dce.h>
#define HAVE_MAC 1
#elif defined(__APPLE__)
/* Native build (the Intel helper): CoreGraphics is linked directly. */
#define HAVE_MAC 1
#define NATIVE_MAC 1
#endif

#ifdef HAVE_MAC

typedef struct { double x, y; } CGPoint_;
typedef struct { CGPoint_ origin; CGPoint_ size; } CGRect_;

enum {
	kCGEventLeftMouseDown = 1, kCGEventLeftMouseUp = 2,
	kCGEventRightMouseDown = 3, kCGEventRightMouseUp = 4,
	kCGEventMouseMoved = 5, kCGEventLeftMouseDragged = 6, kCGEventRightMouseDragged = 7,
	kCGEventKeyDown = 10, kCGEventKeyUp = 11,
	kCGEventScrollWheel = 22,
	kCGEventOtherMouseDown = 25, kCGEventOtherMouseUp = 26, kCGEventOtherMouseDragged = 27,
	kCGMouseButtonLeft = 0, kCGMouseButtonRight = 1, kCGMouseButtonCenter = 2,
	kCGHIDEventTap = 0,
	kCGScrollEventUnitLine = 1,
	kCGMouseEventClickState = 1,
};

static uint32_t (*pCGMainDisplayID)(void);
static CGRect_ (*pCGDisplayBounds)(uint32_t);
static void *(*pCGEventCreate)(void *);
static CGPoint_ (*pCGEventGetLocation)(void *);
static void *(*pCGEventCreateMouseEvent)(void *, uint32_t, CGPoint_, uint32_t);
static void *(*pCGEventCreateKeyboardEvent)(void *, uint16_t, bool);
static void *(*pCGEventCreateScrollWheelEvent2)(void *, uint32_t, uint32_t, int32_t, int32_t, int32_t);
static void (*pCGEventPost)(uint32_t, void *);
static void (*pCGEventSetIntegerValueField)(void *, uint32_t, int64_t);
static void (*pCFRelease)(const void *);

#ifdef NATIVE_MAC
/* Prototypes declared by hand so the same enum names/types serve both builds. */
extern uint32_t CGMainDisplayID(void);
extern CGRect_ CGDisplayBounds(uint32_t);
extern void *CGEventCreate(void *);
extern CGPoint_ CGEventGetLocation(void *);
extern void *CGEventCreateMouseEvent(void *, uint32_t, CGPoint_, uint32_t);
extern void *CGEventCreateKeyboardEvent(void *, uint16_t, bool);
extern void *CGEventCreateScrollWheelEvent2(void *, uint32_t, uint32_t, int32_t, int32_t, int32_t);
extern void CGEventPost(uint32_t, void *);
extern void CGEventSetIntegerValueField(void *, uint32_t, int64_t);
extern void CFRelease(const void *);
extern bool AXIsProcessTrusted(void);
#endif

static CGPoint_ cur;
static bool held_left, held_right;
static int held_other = -1;		/* CG button number (>= 2) of the latest held "other" button */
static uint32_t held_other_mask;	/* bit per CG button number 2..31 */
static int32_t scroll_acc_x, scroll_acc_y;

#define LOADSYM(handle, var, name) \
	do { var = cosmo_dltramp(cosmo_dlsym(handle, name)); \
	     if (!var) { LOGE("CoreGraphics: missing %s\n", name); return -1; } } while (0)

static int mac_init(uint32_t *w, uint32_t *h)
{
#ifdef NATIVE_MAC
	pCGMainDisplayID = CGMainDisplayID;
	pCGDisplayBounds = CGDisplayBounds;
	pCGEventCreate = CGEventCreate;
	pCGEventGetLocation = CGEventGetLocation;
	pCGEventCreateMouseEvent = CGEventCreateMouseEvent;
	pCGEventCreateKeyboardEvent = CGEventCreateKeyboardEvent;
	pCGEventCreateScrollWheelEvent2 = CGEventCreateScrollWheelEvent2;
	pCGEventPost = CGEventPost;
	pCGEventSetIntegerValueField = CGEventSetIntegerValueField;
	pCFRelease = CFRelease;
	if (!AXIsProcessTrusted())
		LOGE("this program is not trusted for Accessibility; events may be ignored "
		     "(System Settings > Privacy & Security > Accessibility)\n");
#else
	if (!IsXnuSilicon()) {
		LOGE("macOS playback is only supported on Apple Silicon (dlopen limitation)\n");
		return -1;
	}
	void *cg = cosmo_dlopen("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_NOW);
	void *cf = cosmo_dlopen("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation", RTLD_NOW);
	if (!cg || !cf) {
		LOGE("cannot load CoreGraphics/CoreFoundation: %s\n", cosmo_dlerror());
		return -1;
	}
	LOADSYM(cg, pCGMainDisplayID, "CGMainDisplayID");
	LOADSYM(cg, pCGDisplayBounds, "CGDisplayBounds");
	LOADSYM(cg, pCGEventCreate, "CGEventCreate");
	LOADSYM(cg, pCGEventGetLocation, "CGEventGetLocation");
	LOADSYM(cg, pCGEventCreateMouseEvent, "CGEventCreateMouseEvent");
	LOADSYM(cg, pCGEventCreateKeyboardEvent, "CGEventCreateKeyboardEvent");
	LOADSYM(cg, pCGEventCreateScrollWheelEvent2, "CGEventCreateScrollWheelEvent2");
	LOADSYM(cg, pCGEventPost, "CGEventPost");
	LOADSYM(cg, pCGEventSetIntegerValueField, "CGEventSetIntegerValueField");
	LOADSYM(cf, pCFRelease, "CFRelease");
#endif

	/* Event locations are display points, so report the size in points too
	 * (CGDisplayBounds), not Retina pixels, or scaling would double up. */
	CGRect_ bounds = pCGDisplayBounds(pCGMainDisplayID());
	*w = (uint32_t)bounds.size.x;
	*h = (uint32_t)bounds.size.y;
	void *probe = pCGEventCreate(0);
	if (probe) {
		cur = pCGEventGetLocation(probe);
		pCFRelease(probe);
	}
	return 0;
}

static void post_mouse(uint32_t type, uint32_t button)
{
	void *ev = pCGEventCreateMouseEvent(0, type, cur, button);
	if (!ev)
		return;
	pCGEventPost(kCGHIDEventTap, ev);
	pCFRelease(ev);
}

static void mac_move_to(double x, double y)
{
	cur.x = x;
	cur.y = y;
	uint32_t type = kCGEventMouseMoved, button = kCGMouseButtonLeft;
	if (held_left) type = kCGEventLeftMouseDragged;
	else if (held_right) { type = kCGEventRightMouseDragged; button = kCGMouseButtonRight; }
	else if (held_other >= 0) { type = kCGEventOtherMouseDragged; button = (uint32_t)held_other; }
	post_mouse(type, button);
}

static void mac_move_abs(int32_t x, int32_t y) { mac_move_to(x, y); }
static void mac_move_rel(int32_t dx, int32_t dy) { mac_move_to(cur.x + dx, cur.y + dy); }

static void mac_button(uint16_t code, bool pressed)
{
	uint32_t type, button;
	switch (code) {
	case ATBSWP_BTN_LEFT:
		type = pressed ? kCGEventLeftMouseDown : kCGEventLeftMouseUp; button = kCGMouseButtonLeft; held_left = pressed; break;
	case ATBSWP_BTN_RIGHT:
		type = pressed ? kCGEventRightMouseDown : kCGEventRightMouseUp; button = kCGMouseButtonRight; held_right = pressed; break;
	case ATBSWP_BTN_MIDDLE:
		type = pressed ? kCGEventOtherMouseDown : kCGEventOtherMouseUp; button = kCGMouseButtonCenter; break;
	case ATBSWP_BTN_SIDE:
		type = pressed ? kCGEventOtherMouseDown : kCGEventOtherMouseUp; button = 3; break;
	case ATBSWP_BTN_EXTRA:
		type = pressed ? kCGEventOtherMouseDown : kCGEventOtherMouseUp; button = 4; break;
	default:
		LOGV("unmapped button %u\n", code);
		return;
	}
	if (button >= 2) {
		/* each "other" button is tracked on its own; drags use the latest one still held */
		if (pressed) {
			held_other_mask |= 1u << button;
			held_other = (int)button;
		} else {
			held_other_mask &= ~(1u << button);
			held_other = -1;
			for (int b = 31; b >= 2; b--)
				if (held_other_mask & (1u << b)) { held_other = b; break; }
		}
	}
	void *ev = pCGEventCreateMouseEvent(0, type, cur, button);
	if (!ev)
		return;
	pCGEventSetIntegerValueField(ev, kCGMouseEventClickState, 1);
	pCGEventPost(kCGHIDEventTap, ev);
	pCFRelease(ev);
}

static void mac_key(uint16_t code, bool pressed)
{
	uint16_t vk;
	if (!keymap_evdev_to_mac(code, &vk)) {
		LOGV("unmapped key %u\n", code);
		return;
	}
	void *ev = pCGEventCreateKeyboardEvent(0, vk, pressed);
	if (!ev)
		return;
	pCGEventPost(kCGHIDEventTap, ev);
	pCFRelease(ev);
}

static void mac_scroll(int32_t x, int32_t y)
{
	/* CoreGraphics line units: positive = up/left. Ours: positive = down/right. */
	scroll_acc_x += x;
	scroll_acc_y += y;
	int32_t lines_y = scroll_acc_y / 120, lines_x = scroll_acc_x / 120;
	if (!lines_x && !lines_y)
		return;
	scroll_acc_y -= lines_y * 120;
	scroll_acc_x -= lines_x * 120;
	void *ev = pCGEventCreateScrollWheelEvent2(0, kCGScrollEventUnitLine, 2, -lines_y, -lines_x, 0);
	if (!ev)
		return;
	pCGEventPost(kCGHIDEventTap, ev);
	pCFRelease(ev);
}

static void mac_shutdown(void) {}

const struct injector injector_macos = {
	.name = "macos",
	.init = mac_init,
	.move_abs = mac_move_abs,
	.move_rel = mac_move_rel,
	.button = mac_button,
	.key = mac_key,
	.scroll = mac_scroll,
	.idle = 0,
	.shutdown = mac_shutdown,
};

#else /* !HAVE_MAC */

static int mac_init(uint32_t *w, uint32_t *h)
{
	(void)w; (void)h;
	LOGE("macOS backend not compiled into this build\n");
	return -1;
}
static void mac_nop2(int32_t a, int32_t b) { (void)a; (void)b; }
static void mac_nopb(uint16_t a, bool b) { (void)a; (void)b; }
static void mac_nop(void) {}
const struct injector injector_macos = {
	.name = "macos", .init = mac_init, .move_abs = mac_nop2, .move_rel = mac_nop2,
	.button = mac_nopb, .key = mac_nopb, .scroll = mac_nop2, .idle = 0, .shutdown = mac_nop,
};

#endif
