/* Prints what would be injected. Works everywhere; used by tests. */
#include <stdio.h>
#include "player.h"

static int dr_init(uint32_t *w, uint32_t *h)
{
	*w = 0;
	*h = 0;
	return 0;
}
static void dr_move_abs(int32_t x, int32_t y) { printf("move %d %d\n", x, y); }
static void dr_move_rel(int32_t x, int32_t y) { printf("moverel %d %d\n", x, y); }
static void dr_button(uint16_t c, bool p) { printf("%s %u\n", p ? "buttondown" : "buttonup", c); }
static void dr_key(uint16_t c, bool p) { printf("%s %u\n", p ? "keydown" : "keyup", c); }
static void dr_scroll(int32_t x, int32_t y) { printf("scroll %d %d\n", x, y); }
static void dr_shutdown(void) { fflush(stdout); }

const struct injector injector_dry_run = {
	.name = "dry-run",
	.init = dr_init,
	.move_abs = dr_move_abs,
	.move_rel = dr_move_rel,
	.button = dr_button,
	.key = dr_key,
	.scroll = dr_scroll,
	.idle = 0,
	.shutdown = dr_shutdown,
};
