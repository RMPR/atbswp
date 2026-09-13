/* evdev keycode -> Windows scancode / macOS virtual keycode. */
#include "player.h"

struct keymap_entry {
	uint16_t evdev;
	uint8_t win_sc;		/* 0 = no mapping */
	uint8_t win_ext;	/* needs E0 prefix */
	uint8_t mac_vk;		/* 0xFF = no mapping */
};

#define NOMAC 0xFF

static const struct keymap_entry table[] = {
	/* evdev, win scancode, ext, mac vk */
	{1, 0x01, 0, 0x35},	/* ESC */
	{2, 0x02, 0, 0x12},	/* 1 */
	{3, 0x03, 0, 0x13},	/* 2 */
	{4, 0x04, 0, 0x14},	/* 3 */
	{5, 0x05, 0, 0x15},	/* 4 */
	{6, 0x06, 0, 0x17},	/* 5 */
	{7, 0x07, 0, 0x16},	/* 6 */
	{8, 0x08, 0, 0x1A},	/* 7 */
	{9, 0x09, 0, 0x1C},	/* 8 */
	{10, 0x0A, 0, 0x19},	/* 9 */
	{11, 0x0B, 0, 0x1D},	/* 0 */
	{12, 0x0C, 0, 0x1B},	/* MINUS */
	{13, 0x0D, 0, 0x18},	/* EQUAL */
	{14, 0x0E, 0, 0x33},	/* BACKSPACE */
	{15, 0x0F, 0, 0x30},	/* TAB */
	{16, 0x10, 0, 0x0C},	/* Q */
	{17, 0x11, 0, 0x0D},	/* W */
	{18, 0x12, 0, 0x0E},	/* E */
	{19, 0x13, 0, 0x0F},	/* R */
	{20, 0x14, 0, 0x11},	/* T */
	{21, 0x15, 0, 0x10},	/* Y */
	{22, 0x16, 0, 0x20},	/* U */
	{23, 0x17, 0, 0x22},	/* I */
	{24, 0x18, 0, 0x1F},	/* O */
	{25, 0x19, 0, 0x23},	/* P */
	{26, 0x1A, 0, 0x21},	/* LEFTBRACE */
	{27, 0x1B, 0, 0x1E},	/* RIGHTBRACE */
	{28, 0x1C, 0, 0x24},	/* ENTER */
	{29, 0x1D, 0, 0x3B},	/* LEFTCTRL */
	{30, 0x1E, 0, 0x00},	/* A */
	{31, 0x1F, 0, 0x01},	/* S */
	{32, 0x20, 0, 0x02},	/* D */
	{33, 0x21, 0, 0x03},	/* F */
	{34, 0x22, 0, 0x05},	/* G */
	{35, 0x23, 0, 0x04},	/* H */
	{36, 0x24, 0, 0x26},	/* J */
	{37, 0x25, 0, 0x28},	/* K */
	{38, 0x26, 0, 0x25},	/* L */
	{39, 0x27, 0, 0x29},	/* SEMICOLON */
	{40, 0x28, 0, 0x27},	/* APOSTROPHE */
	{41, 0x29, 0, 0x32},	/* GRAVE */
	{42, 0x2A, 0, 0x38},	/* LEFTSHIFT */
	{43, 0x2B, 0, 0x2A},	/* BACKSLASH */
	{44, 0x2C, 0, 0x06},	/* Z */
	{45, 0x2D, 0, 0x07},	/* X */
	{46, 0x2E, 0, 0x08},	/* C */
	{47, 0x2F, 0, 0x09},	/* V */
	{48, 0x30, 0, 0x0B},	/* B */
	{49, 0x31, 0, 0x2D},	/* N */
	{50, 0x32, 0, 0x2E},	/* M */
	{51, 0x33, 0, 0x2B},	/* COMMA */
	{52, 0x34, 0, 0x2F},	/* DOT */
	{53, 0x35, 0, 0x2C},	/* SLASH */
	{54, 0x36, 0, 0x3C},	/* RIGHTSHIFT */
	{55, 0x37, 0, 0x43},	/* KPASTERISK */
	{56, 0x38, 0, 0x3A},	/* LEFTALT */
	{57, 0x39, 0, 0x31},	/* SPACE */
	{58, 0x3A, 0, 0x39},	/* CAPSLOCK */
	{59, 0x3B, 0, 0x7A},	/* F1 */
	{60, 0x3C, 0, 0x78},	/* F2 */
	{61, 0x3D, 0, 0x63},	/* F3 */
	{62, 0x3E, 0, 0x76},	/* F4 */
	{63, 0x3F, 0, 0x60},	/* F5 */
	{64, 0x40, 0, 0x61},	/* F6 */
	{65, 0x41, 0, 0x62},	/* F7 */
	{66, 0x42, 0, 0x64},	/* F8 */
	{67, 0x43, 0, 0x65},	/* F9 */
	{68, 0x44, 0, 0x6D},	/* F10 */
	{69, 0x45, 0, 0x47},	/* NUMLOCK -> mac keypad clear */
	{70, 0x46, 0, NOMAC},	/* SCROLLLOCK */
	{71, 0x47, 0, 0x59},	/* KP7 */
	{72, 0x48, 0, 0x5B},	/* KP8 */
	{73, 0x49, 0, 0x5C},	/* KP9 */
	{74, 0x4A, 0, 0x4E},	/* KPMINUS */
	{75, 0x4B, 0, 0x56},	/* KP4 */
	{76, 0x4C, 0, 0x57},	/* KP5 */
	{77, 0x4D, 0, 0x58},	/* KP6 */
	{78, 0x4E, 0, 0x45},	/* KPPLUS */
	{79, 0x4F, 0, 0x53},	/* KP1 */
	{80, 0x50, 0, 0x54},	/* KP2 */
	{81, 0x51, 0, 0x55},	/* KP3 */
	{82, 0x52, 0, 0x52},	/* KP0 */
	{83, 0x53, 0, 0x41},	/* KPDOT */
	{86, 0x56, 0, 0x0A},	/* 102ND (ISO section) */
	{87, 0x57, 0, 0x67},	/* F11 */
	{88, 0x58, 0, 0x6F},	/* F12 */
	{96, 0x1C, 1, 0x4C},	/* KPENTER */
	{97, 0x1D, 1, 0x3E},	/* RIGHTCTRL */
	{98, 0x35, 1, 0x4B},	/* KPSLASH */
	{99, 0x37, 1, 0x69},	/* SYSRQ / print screen -> F13 */
	{100, 0x38, 1, 0x3D},	/* RIGHTALT */
	{102, 0x47, 1, 0x73},	/* HOME */
	{103, 0x48, 1, 0x7E},	/* UP */
	{104, 0x49, 1, 0x74},	/* PAGEUP */
	{105, 0x4B, 1, 0x7B},	/* LEFT */
	{106, 0x4D, 1, 0x7C},	/* RIGHT */
	{107, 0x4F, 1, 0x77},	/* END */
	{108, 0x50, 1, 0x7D},	/* DOWN */
	{109, 0x51, 1, 0x79},	/* PAGEDOWN */
	{110, 0x52, 1, 0x72},	/* INSERT -> mac help */
	{111, 0x53, 1, 0x75},	/* DELETE (forward) */
	{113, 0x20, 1, 0x4A},	/* MUTE */
	{114, 0x2E, 1, 0x49},	/* VOLUMEDOWN */
	{115, 0x30, 1, 0x48},	/* VOLUMEUP */
	{117, 0x59, 0, 0x51},	/* KPEQUAL */
	{119, 0x45, 1, NOMAC},	/* PAUSE (approximation) */
	{125, 0x5B, 1, 0x37},	/* LEFTMETA -> command */
	{126, 0x5C, 1, 0x36},	/* RIGHTMETA -> right command */
	{127, 0x5D, 1, NOMAC},	/* COMPOSE / menu */
	{183, 0x64, 0, 0x69},	/* F13 */
	{184, 0x65, 0, 0x6B},	/* F14 */
	{185, 0x66, 0, 0x71},	/* F15 */
	{186, 0x67, 0, 0x6A},	/* F16 */
	{187, 0x68, 0, 0x40},	/* F17 */
	{188, 0x69, 0, 0x4F},	/* F18 */
	{189, 0x6A, 0, 0x50},	/* F19 */
	{190, 0x6B, 0, 0x5A},	/* F20 */
};

static const struct keymap_entry *lookup(uint16_t evdev)
{
	for (size_t i = 0; i < sizeof(table) / sizeof(table[0]); i++)
		if (table[i].evdev == evdev)
			return &table[i];
	return 0;
}

bool keymap_evdev_to_win(uint16_t evdev, uint16_t *scancode, bool *extended)
{
	const struct keymap_entry *e = lookup(evdev);
	if (!e || !e->win_sc)
		return false;
	*scancode = e->win_sc;
	*extended = e->win_ext;
	return true;
}

bool keymap_evdev_to_mac(uint16_t evdev, uint16_t *vk)
{
	const struct keymap_entry *e = lookup(evdev);
	if (!e || e->mac_vk == NOMAC)
		return false;
	*vk = e->mac_vk;
	return true;
}
