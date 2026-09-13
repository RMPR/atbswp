#ifndef ATBSWP_PLAYER_H
#define ATBSWP_PLAYER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdint.h>
#include "macro_format.h"

/* A backend injects input on one platform.  All coordinates handed to
 * move_abs are already scaled to the target screen by the core. */
struct injector {
	const char *name;
	/* Called once. Must fill screen_w/screen_h (0 if unknown). */
	int (*init)(uint32_t *screen_w, uint32_t *screen_h);
	void (*move_abs)(int32_t x, int32_t y);
	void (*move_rel)(int32_t dx, int32_t dy);
	void (*button)(uint16_t evdev_code, bool pressed);
	void (*key)(uint16_t evdev_code, bool pressed);
	void (*scroll)(int32_t x120, int32_t y120);
	/* Called while waiting between events, with the remaining wait in
	 * microseconds. Backends with a connection to service should poll
	 * it here and return early on error (return non-zero). May be NULL. */
	int (*idle)(uint32_t us);
	void (*shutdown)(void);
};

extern const struct injector injector_linux;
extern const struct injector injector_windows;
extern const struct injector injector_macos;
extern const struct injector injector_dry_run;

extern int verbose;
#define LOGV(...) do { if (verbose) fprintf(stderr, "atbswp: " __VA_ARGS__); } while (0)
#define LOGE(...) fprintf(stderr, "atbswp: " __VA_ARGS__)

/* keymap.c */
/* Translate an evdev keycode to a Windows set-1 scancode. Returns false if
 * unmapped. *extended is set when the E0 prefix is required. */
bool keymap_evdev_to_win(uint16_t evdev, uint16_t *scancode, bool *extended);
/* Translate an evdev keycode to a macOS virtual keycode. */
bool keymap_evdev_to_mac(uint16_t evdev, uint16_t *vk);

/* util */
void sleep_us(uint64_t us);
uint64_t now_us(void);

#endif
