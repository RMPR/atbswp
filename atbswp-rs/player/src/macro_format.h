/* atbswp macro payload format, version 1.
 *
 * This header is the single source of truth for the binary layout shared by
 * the Rust recorder (crates/atbswp-macro/src/format.rs mirrors it) and the C
 * player.  Everything is little-endian and packed with natural alignment, so
 * the structs below can be read straight from disk on every supported target.
 *
 * File layout of a standalone macro executable:
 *
 *   +------------------------------------------+
 *   | player.com (this program, APE fat binary)|
 *   +------------------------------------------+
 *   | atbswp_header           (32 bytes)       |
 *   | atbswp_event[n]         (16 bytes each)  |
 *   +------------------------------------------+
 *   | uint64_t payload_len    (header+events)  |
 *   | char     magic[8]  = "ATBSWPM1"          |
 *   +------------------------------------------+
 */
#ifndef ATBSWP_MACRO_FORMAT_H
#define ATBSWP_MACRO_FORMAT_H

#include <stdint.h>

#define ATBSWP_MAGIC "ATBSWPM1"
#define ATBSWP_MAGIC_LEN 8
#define ATBSWP_FOOTER_LEN 16
#define ATBSWP_FORMAT_VERSION 1

enum atbswp_event_type {
	ATBSWP_EV_MOVE_ABS = 1,		/* x,y: absolute position in recorded screen pixels */
	ATBSWP_EV_MOVE_REL = 2,		/* x,y: relative delta in pixels */
	ATBSWP_EV_BUTTON_PRESS = 3,	/* code: evdev BTN_* (0x110 = left) */
	ATBSWP_EV_BUTTON_RELEASE = 4,
	ATBSWP_EV_KEY_PRESS = 5,	/* code: evdev KEY_* */
	ATBSWP_EV_KEY_RELEASE = 6,
	ATBSWP_EV_SCROLL = 7,		/* x,y: 1/120th of a wheel notch, +y = down, +x = right */
};

typedef struct {
	uint16_t type;		/* enum atbswp_event_type */
	uint16_t code;		/* key / button code (evdev numbering) */
	int32_t x;
	int32_t y;
	uint32_t delay_us;	/* time to wait *before* this event */
} atbswp_event;

typedef struct {
	uint16_t version;	/* ATBSWP_FORMAT_VERSION */
	uint16_t flags;		/* reserved, 0 */
	uint32_t event_count;
	uint32_t screen_w;	/* recording screen size, 0 = unknown (no scaling) */
	uint32_t screen_h;
	uint32_t repeat;	/* default repeat count, 0 = forever */
	uint32_t speed_percent;	/* default playback speed, 100 = real time */
	uint32_t reserved[2];
} atbswp_header;

/* evdev button codes we care about */
#define ATBSWP_BTN_LEFT    0x110
#define ATBSWP_BTN_RIGHT   0x111
#define ATBSWP_BTN_MIDDLE  0x112
#define ATBSWP_BTN_SIDE    0x113
#define ATBSWP_BTN_EXTRA   0x114

#endif
