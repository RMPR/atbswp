/* Minimal reader for stored (uncompressed) entries of a zip archive that
 * sits at the end of an arbitrary file, e.g. an executable.  Used by the
 * native macOS helper, which has no cosmopolitan /zip/ filesystem, and for
 * `--payload FILE` when FILE is an exported macro. */
#include <string.h>
#include "player.h"

static uint16_t rd16(const uint8_t *p) { return (uint16_t)(p[0] | p[1] << 8); }
static uint32_t rd32(const uint8_t *p) { return (uint32_t)p[0] | (uint32_t)p[1] << 8 | (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24; }

bool zip_find(const uint8_t *buf, size_t len, const char *name, size_t *off, size_t *size)
{
	/* End of central directory: scan back over the comment (<= 64 KiB). */
	if (len < 22)
		return false;
	size_t eocd = 0;
	bool found = false;
	size_t stop = len > 22 + 65535 ? len - 22 - 65535 : 0;
	for (size_t i = len - 22; ; i--) {
		if (rd32(buf + i) == 0x06054b50) {
			eocd = i;
			found = true;
			break;
		}
		if (i == stop)
			break;
	}
	if (!found)
		return false;
	uint16_t count = rd16(buf + eocd + 10);
	uint32_t cd_size = rd32(buf + eocd + 12);
	uint32_t cd_off = rd32(buf + eocd + 16);
	if ((size_t)cd_off + cd_size > eocd)
		return false;
	size_t namelen = strlen(name);
	size_t p = cd_off;
	for (uint16_t n = 0; n < count; n++) {
		if (p + 46 > eocd || rd32(buf + p) != 0x02014b50)
			return false;
		uint16_t method = rd16(buf + p + 10);
		uint32_t usize = rd32(buf + p + 24);
		uint16_t nlen = rd16(buf + p + 28), xlen = rd16(buf + p + 30), clen = rd16(buf + p + 32);
		uint32_t lho = rd32(buf + p + 42);
		if (nlen == namelen && !memcmp(buf + p + 46, name, namelen)) {
			if (method != 0 || (size_t)lho + 30 > len || rd32(buf + lho) != 0x04034b50)
				return false;
			size_t data = (size_t)lho + 30 + rd16(buf + lho + 26) + rd16(buf + lho + 28);
			if (data + usize > len)
				return false;
			*off = data;
			*size = usize;
			return true;
		}
		p += 46 + nlen + xlen + clen;
	}
	return false;
}
