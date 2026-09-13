# Builds the standalone macro player as an Actually Portable Executable.
#
#   make            -> build/player.com (fat x86_64 + aarch64, all OSes)
#   make native     -> build/player-native (host gcc, Linux only, for quick tests)
#   make test       -> unit-ish checks that need no display
#
# cosmocc is downloaded into .cosmocc/ on first use unless COSMOCC points at
# an existing toolchain's bin/cosmocc.

COSMOCC_VERSION ?= latest
COSMOCC_URL ?= https://cosmo.zip/pub/cosmocc/cosmocc.zip
COSMOCC ?= .cosmocc/bin/cosmocc

SRC := src/player.c src/keymap.c src/zipread.c src/backend_dry_run.c src/backend_linux.c \
       src/backend_windows.c src/backend_macos.c
HDR := src/player.h src/macro_format.h

CFLAGS ?= -Os -Wall -Wextra -Wno-unused-parameter
# -mtiny: smallest cosmo runtime; -s: no embedded symbol tables (~120 KiB)
COSMOFLAGS ?= -mcosmo -mtiny -s

all: build/player.com

$(COSMOCC):
	@echo "Fetching cosmocc toolchain..."
	@mkdir -p .cosmocc
	@curl -fsSL -o .cosmocc/cosmocc.zip $(COSMOCC_URL)
	@cd .cosmocc && unzip -qo cosmocc.zip && rm cosmocc.zip
	@test -x $(COSMOCC)

build/player.com: $(SRC) $(HDR) | $(COSMOCC)
	@mkdir -p build
	$(COSMOCC) $(COSMOFLAGS) $(CFLAGS) -o $@ $(SRC)
	@ls -l $@

native: build/player-native
build/player-native: $(SRC) $(HDR)
	@mkdir -p build
	$(CC) -std=gnu11 $(CFLAGS) -o $@ $(SRC) -ldl

# Native x86-64 macOS build of the player, bundled into the APE so Intel Macs
# (where cosmopolitan cannot dlopen) can play too.  Built on a macOS host.
native-macos: build/player-macos-x86_64
build/player-macos-x86_64: $(SRC) $(HDR)
	@mkdir -p build
	clang -arch x86_64 -mmacosx-version-min=10.13 -std=gnu11 $(CFLAGS) -o $@ $(SRC) \
		-framework CoreGraphics -framework CoreFoundation -framework ApplicationServices
	strip $@

# Store the macOS helper in the APE's zip section (APEs are zip files):
#   make bundle HELPER=path/to/player-macos-x86_64
HELPER ?= build/player-macos-x86_64
bundle: build/player.com
	python3 tools/bundle.py build/player.com "$(HELPER)"
	@ls -l build/player.com

# A tiny EIS server that logs everything it receives; used by tests/e2e.sh.
build/eis_sink: tests/eis_sink.c
	@mkdir -p build
	$(CC) -std=gnu11 -Wall -O1 -I tests/third_party -o $@ $< -l:libeis.so.1

test: build/player.com
	sh tests/smoke.sh build/player.com

test-native: build/player-native
	sh tests/smoke.sh build/player-native

e2e: build/player.com build/eis_sink
	sh ../tests/e2e.sh

clean:
	rm -rf build

distclean: clean
	rm -rf .cosmocc

.PHONY: all native native-macos bundle test test-native e2e clean distclean
