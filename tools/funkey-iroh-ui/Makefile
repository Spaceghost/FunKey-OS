CC ?= cc
CFLAGS ?= -Os
CPPFLAGS ?=
LDFLAGS ?=
LDLIBS ?= -lSDL_ttf -lSDL -lasound -lm

CFLAGS += -std=gnu99 -Wall -Wextra -ffunction-sections -fdata-sections
LDFLAGS += -Wl,--gc-sections

.PHONY: all clean

all: funkey-iroh-ui

funkey-iroh-ui: main.c ui-core.inc ui-qr.inc ui-settings.inc ui-launch.inc
	$(CC) $(CPPFLAGS) $(CFLAGS) -o $@ $< $(LDFLAGS) $(LDLIBS)

clean:
	rm -f funkey-iroh-ui
