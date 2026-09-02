#define _POSIX_C_SOURCE 200809L

#include <SDL/SDL.h>
#include <SDL/SDL_ttf.h>

#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#ifndef SDL_TRIPLEBUF
#define SDL_TRIPLEBUF SDL_DOUBLEBUF
#endif

#define MAX_OUTPUT 32768
#define MAX_ITEMS 96
#define MAX_LABEL 128
#define MAX_DETAIL 256
#define MAX_TICKET 2048
#define ACTION_FILE "/var/run/funkey-iroh/launch.action"
#define FONT_PATH "/usr/share/fonts/droid/DroidSansFallback.ttf"
#define FONT_PATH_FULL "/usr/share/fonts/droid/DroidSansFallbackFull.ttf"

typedef enum {
    KEY_NONE = 0,
    KEY_UP,
    KEY_DOWN,
    KEY_LEFT,
    KEY_RIGHT,
    KEY_SELECT,
    KEY_BACK,
    KEY_QUIT
} UiKey;

typedef struct {
    char label[MAX_LABEL];
    char detail[MAX_DETAIL];
    int enabled;
    int checked;
} MenuItem;

typedef struct {
    char name[65];
    char id[96];
    int sync;
} Peer;

typedef struct {
    char path[PATH_MAX];
    char peer[65];
    char system[97];
    char game[97];
    char filename[128];
    char target[PATH_MAX];
    unsigned long long size;
} InboxItem;

typedef struct {
    SDL_Surface *screen;
    TTF_Font *title_font;
    TTF_Font *body_font;
    TTF_Font *small_font;
    Uint32 background;
    Uint32 panel;
    Uint32 selected;
    Uint32 foreground;
    Uint32 muted;
    Uint32 warning;
    Uint32 success;
    int width;
    int height;
} Ui;

static Ui ui;

#include "ui-core.inc"
#include "ui-qr.inc"
#include "ui-settings.inc"
#include "ui-launch.inc"
