#include <SDL/SDL.h>

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SCREEN_FALLBACK 240
#define MENU_COUNT 13
#define MESSAGE_LINES 13
#define MESSAGE_COLS 35

struct glyph {
    char ch;
    uint8_t row[7];
};

static const struct glyph font[] = {
    {' ', {0, 0, 0, 0, 0, 0, 0}},
    {'!', {4, 4, 4, 4, 4, 0, 4}},
    {'"', {10, 10, 0, 0, 0, 0, 0}},
    {'#', {10, 31, 10, 10, 31, 10, 0}},
    {'%', {17, 2, 4, 8, 17, 0, 0}},
    {'&', {6, 9, 10, 4, 21, 18, 13}},
    {'\'', {4, 4, 0, 0, 0, 0, 0}},
    {'(', {2, 4, 8, 8, 8, 4, 2}},
    {')', {8, 4, 2, 2, 2, 4, 8}},
    {'*', {0, 21, 14, 31, 14, 21, 0}},
    {'+', {0, 4, 4, 31, 4, 4, 0}},
    {',', {0, 0, 0, 0, 0, 4, 8}},
    {'-', {0, 0, 0, 31, 0, 0, 0}},
    {'.', {0, 0, 0, 0, 0, 0, 4}},
    {'/', {1, 2, 4, 8, 16, 0, 0}},
    {'0', {14, 17, 19, 21, 25, 17, 14}},
    {'1', {4, 12, 4, 4, 4, 4, 14}},
    {'2', {14, 17, 1, 2, 4, 8, 31}},
    {'3', {30, 1, 1, 14, 1, 1, 30}},
    {'4', {2, 6, 10, 18, 31, 2, 2}},
    {'5', {31, 16, 16, 30, 1, 1, 30}},
    {'6', {6, 8, 16, 30, 17, 17, 14}},
    {'7', {31, 1, 2, 4, 8, 8, 8}},
    {'8', {14, 17, 17, 14, 17, 17, 14}},
    {'9', {14, 17, 17, 15, 1, 2, 12}},
    {':', {0, 4, 0, 0, 4, 0, 0}},
    {';', {0, 4, 0, 0, 4, 4, 8}},
    {'<', {2, 4, 8, 16, 8, 4, 2}},
    {'=', {0, 0, 31, 0, 31, 0, 0}},
    {'>', {8, 4, 2, 1, 2, 4, 8}},
    {'?', {14, 17, 1, 2, 4, 0, 4}},
    {'@', {14, 17, 23, 21, 23, 16, 14}},
    {'A', {14, 17, 17, 31, 17, 17, 17}},
    {'B', {30, 17, 17, 30, 17, 17, 30}},
    {'C', {14, 17, 16, 16, 16, 17, 14}},
    {'D', {30, 17, 17, 17, 17, 17, 30}},
    {'E', {31, 16, 16, 30, 16, 16, 31}},
    {'F', {31, 16, 16, 30, 16, 16, 16}},
    {'G', {14, 17, 16, 23, 17, 17, 15}},
    {'H', {17, 17, 17, 31, 17, 17, 17}},
    {'I', {14, 4, 4, 4, 4, 4, 14}},
    {'J', {7, 2, 2, 2, 2, 18, 12}},
    {'K', {17, 18, 20, 24, 20, 18, 17}},
    {'L', {16, 16, 16, 16, 16, 16, 31}},
    {'M', {17, 27, 21, 21, 17, 17, 17}},
    {'N', {17, 25, 21, 19, 17, 17, 17}},
    {'O', {14, 17, 17, 17, 17, 17, 14}},
    {'P', {30, 17, 17, 30, 16, 16, 16}},
    {'Q', {14, 17, 17, 17, 21, 18, 13}},
    {'R', {30, 17, 17, 30, 20, 18, 17}},
    {'S', {15, 16, 16, 14, 1, 1, 30}},
    {'T', {31, 4, 4, 4, 4, 4, 4}},
    {'U', {17, 17, 17, 17, 17, 17, 14}},
    {'V', {17, 17, 17, 17, 17, 10, 4}},
    {'W', {17, 17, 17, 21, 21, 21, 10}},
    {'X', {17, 17, 10, 4, 10, 17, 17}},
    {'Y', {17, 17, 10, 4, 4, 4, 4}},
    {'Z', {31, 1, 2, 4, 8, 16, 31}},
    {'[', {14, 8, 8, 8, 8, 8, 14}},
    {'\\', {16, 8, 4, 2, 1, 0, 0}},
    {']', {14, 2, 2, 2, 2, 2, 14}},
    {'_', {0, 0, 0, 0, 0, 0, 31}},
};

struct summary {
    char service[24];
    char peers[16];
    char default_peer[48];
    char last_game[64];
    char bundles[16];
    char incoming[16];
    char autosync[16];
    char usb_mode[16];
};

static SDL_Surface *screen;
static Uint32 background;
static Uint32 foreground;
static Uint32 selected_bg;
static Uint32 selected_fg;
static Uint32 muted;

static const struct glyph *find_glyph(char ch)
{
    size_t i;
    ch = (char)toupper((unsigned char)ch);
    for (i = 0; i < sizeof(font) / sizeof(font[0]); ++i) {
        if (font[i].ch == ch)
            return &font[i];
    }
    return &font[0];
}

static void fill_rect(int x, int y, int w, int h, Uint32 color)
{
    SDL_Rect rectangle;
    rectangle.x = (Sint16)x;
    rectangle.y = (Sint16)y;
    rectangle.w = (Uint16)w;
    rectangle.h = (Uint16)h;
    SDL_FillRect(screen, &rectangle, color);
}

static void draw_char(int x, int y, int scale, char ch, Uint32 color)
{
    const struct glyph *glyph = find_glyph(ch);
    int row;
    int column;
    for (row = 0; row < 7; ++row) {
        for (column = 0; column < 5; ++column) {
            if (glyph->row[row] & (1u << (4 - column))) {
                fill_rect(
                    x + column * scale,
                    y + row * scale,
                    scale,
                    scale,
                    color
                );
            }
        }
    }
}

static void draw_text(int x, int y, int scale, const char *text, Uint32 color)
{
    int origin = x;
    while (*text) {
        if (*text == '\n') {
            x = origin;
            y += 8 * scale;
        } else {
            draw_char(x, y, scale, *text, color);
            x += 6 * scale;
        }
        ++text;
    }
}

static void copy_value(char *destination, size_t size, const char *value)
{
    if (size == 0)
        return;
    snprintf(destination, size, "%s", value ? value : "");
}

static void summary_defaults(struct summary *value)
{
    memset(value, 0, sizeof(*value));
    copy_value(value->service, sizeof(value->service), "UNKNOWN");
    copy_value(value->peers, sizeof(value->peers), "0");
    copy_value(value->default_peer, sizeof(value->default_peer), "NONE");
    copy_value(value->last_game, sizeof(value->last_game), "NONE");
    copy_value(value->bundles, sizeof(value->bundles), "0");
    copy_value(value->incoming, sizeof(value->incoming), "0");
    copy_value(value->autosync, sizeof(value->autosync), "OFF");
    copy_value(value->usb_mode, sizeof(value->usb_mode), "RNDIS");
}

static void assign_summary(struct summary *value, char *line)
{
    char *separator = strchr(line, '=');
    char *key;
    char *data;
    if (!separator)
        return;
    *separator = '\0';
    key = line;
    data = separator + 1;
    data[strcspn(data, "\r\n")] = '\0';

    if (!strcmp(key, "SERVICE"))
        copy_value(value->service, sizeof(value->service), data);
    else if (!strcmp(key, "PEERS"))
        copy_value(value->peers, sizeof(value->peers), data);
    else if (!strcmp(key, "DEFAULT_PEER"))
        copy_value(value->default_peer, sizeof(value->default_peer), data);
    else if (!strcmp(key, "LAST_GAME"))
        copy_value(value->last_game, sizeof(value->last_game), data);
    else if (!strcmp(key, "BUNDLES"))
        copy_value(value->bundles, sizeof(value->bundles), data);
    else if (!strcmp(key, "INCOMING"))
        copy_value(value->incoming, sizeof(value->incoming), data);
    else if (!strcmp(key, "AUTOSYNC"))
        copy_value(value->autosync, sizeof(value->autosync), data);
    else if (!strcmp(key, "USB_MODE"))
        copy_value(value->usb_mode, sizeof(value->usb_mode), data);
}

static void load_summary(struct summary *value)
{
    FILE *pipe;
    char line[256];
    summary_defaults(value);
    pipe = popen("/usr/bin/funkey-iroh-ui-action summary 2>/dev/null", "r");
    if (!pipe)
        return;
    while (fgets(line, sizeof(line), pipe))
        assign_summary(value, line);
    pclose(pipe);
}

static void truncate_label(char *label, size_t maximum)
{
    size_t length = strlen(label);
    if (length <= maximum)
        return;
    if (maximum < 4) {
        label[maximum] = '\0';
        return;
    }
    label[maximum - 3] = '.';
    label[maximum - 2] = '.';
    label[maximum - 1] = '.';
    label[maximum] = '\0';
}

static void menu_label(
    int index,
    const struct summary *value,
    char *label,
    size_t label_size
)
{
    switch (index) {
    case 0:
        snprintf(label, label_size, "SERVICE: %s", value->service);
        break;
    case 1:
        snprintf(label, label_size, "MY PAIRING QR");
        break;
    case 2:
        snprintf(label, label_size, "IMPORT PAIRING FILES");
        break;
    case 3:
        snprintf(label, label_size, "PEER: %s (%s)", value->default_peer, value->peers);
        break;
    case 4:
        snprintf(label, label_size, "SNAPSHOT: %s", value->last_game);
        break;
    case 5:
        snprintf(label, label_size, "SEND LAST GAME");
        break;
    case 6:
        snprintf(label, label_size, "AUTO-SYNC: %s", value->autosync);
        break;
    case 7:
        snprintf(label, label_size, "INSTALL RECEIVED (%s)", value->incoming);
        break;
    case 8:
        snprintf(label, label_size, "USB NETWORK: %s", value->usb_mode);
        break;
    case 9:
        snprintf(label, label_size, "SFTP / SSHFS DETAILS");
        break;
    case 10:
        snprintf(label, label_size, "NETPLAY TRANSPORT STATUS");
        break;
    case 11:
        snprintf(label, label_size, "DIAGNOSTICS");
        break;
    default:
        snprintf(label, label_size, "EXIT");
        break;
    }
    truncate_label(label, 35);
}

static void render_menu(int selection, const struct summary *value)
{
    int i;
    int line_height = 16;
    int top = 30;
    int visible = (screen->h - top - 20) / line_height;
    int first = selection - visible / 2;
    char label[96];

    if (first < 0)
        first = 0;
    if (first + visible > MENU_COUNT)
        first = MENU_COUNT - visible;
    if (first < 0)
        first = 0;

    SDL_FillRect(screen, NULL, background);
    draw_text(8, 7, 2, "IROH SHARE + PLAY", foreground);
    fill_rect(8, 24, screen->w - 16, 1, muted);

    for (i = first; i < MENU_COUNT && i < first + visible; ++i) {
        int y = top + (i - first) * line_height;
        menu_label(i, value, label, sizeof(label));
        if (i == selection) {
            fill_rect(4, y - 2, screen->w - 8, 15, selected_bg);
            draw_text(8, y, 1, label, selected_fg);
        } else {
            draw_text(8, y, 1, label, foreground);
        }
    }

    draw_text(
        8,
        screen->h - 12,
        1,
        "U/D MOVE  A SELECT  B/Q EXIT",
        muted
    );
    SDL_Flip(screen);
}

static int is_accept(SDLKey key)
{
    return key == SDLK_RETURN || key == SDLK_SPACE || key == SDLK_a;
}

static int is_back(SDLKey key)
{
    return key == SDLK_ESCAPE || key == SDLK_b || key == SDLK_q;
}

static int is_up(SDLKey key)
{
    return key == SDLK_UP || key == SDLK_u;
}

static int is_down(SDLKey key)
{
    return key == SDLK_DOWN || key == SDLK_d;
}

static void wait_for_dismiss(void)
{
    SDL_Event event;
    Uint32 deadline = SDL_GetTicks() + 8000;
    while (SDL_GetTicks() < deadline) {
        while (SDL_PollEvent(&event)) {
            if (event.type == SDL_QUIT)
                return;
            if (event.type == SDL_KEYDOWN)
                return;
        }
        SDL_Delay(20);
    }
}

static void wrap_message(
    FILE *file,
    char lines[MESSAGE_LINES][MESSAGE_COLS + 1],
    int *line_count
)
{
    char word[128];
    size_t current = 0;
    int count = 0;
    int character;

    memset(lines, 0, MESSAGE_LINES * (MESSAGE_COLS + 1));
    memset(word, 0, sizeof(word));

    while ((character = fgetc(file)) != EOF && count < MESSAGE_LINES) {
        if (character == '\r')
            continue;
        if (character == '\n' || isspace((unsigned char)character)) {
            if (current > 0) {
                word[current] = '\0';
                if (lines[count][0]
                    && strlen(lines[count]) + current + 1 > MESSAGE_COLS) {
                    ++count;
                    if (count >= MESSAGE_LINES)
                        break;
                }
                if (lines[count][0])
                    strncat(
                        lines[count],
                        " ",
                        MESSAGE_COLS - strlen(lines[count])
                    );
                strncat(
                    lines[count],
                    word,
                    MESSAGE_COLS - strlen(lines[count])
                );
                current = 0;
            }
            if (character == '\n' && lines[count][0]) {
                ++count;
                if (count >= MESSAGE_LINES)
                    break;
            }
        } else if (current + 1 < sizeof(word)) {
            word[current++] = (char)character;
        }
    }
    if (current > 0 && count < MESSAGE_LINES) {
        word[current] = '\0';
        if (lines[count][0]
            && strlen(lines[count]) + current + 1 > MESSAGE_COLS) {
            ++count;
        }
        if (count < MESSAGE_LINES) {
            if (lines[count][0])
                strncat(
                    lines[count],
                    " ",
                    MESSAGE_COLS - strlen(lines[count])
                );
            strncat(
                lines[count],
                word,
                MESSAGE_COLS - strlen(lines[count])
            );
        }
    }
    if (count < MESSAGE_LINES && lines[count][0])
        ++count;
    *line_count = count;
}

static void show_message(const char *title, const char *path)
{
    FILE *file = fopen(path, "r");
    char lines[MESSAGE_LINES][MESSAGE_COLS + 1];
    int count = 0;
    int i;

    if (file) {
        wrap_message(file, lines, &count);
        fclose(file);
    }
    SDL_FillRect(screen, NULL, background);
    draw_text(8, 8, 2, title, foreground);
    fill_rect(8, 25, screen->w - 16, 1, muted);
    if (count == 0) {
        draw_text(8, 36, 1, "NO OUTPUT", muted);
    } else {
        for (i = 0; i < count; ++i)
            draw_text(8, 36 + i * 14, 1, lines[i], foreground);
    }
    draw_text(8, screen->h - 12, 1, "PRESS ANY BUTTON", muted);
    SDL_Flip(screen);
    wait_for_dismiss();
}

static int pbm_token(FILE *file, char *token, size_t size)
{
    int character;
    size_t used = 0;

    do {
        character = fgetc(file);
        if (character == '#') {
            while (character != '\n' && character != EOF)
                character = fgetc(file);
        }
    } while (character != EOF && isspace((unsigned char)character));

    if (character == EOF)
        return 0;

    while (character != EOF && !isspace((unsigned char)character)) {
        if (used + 1 < size)
            token[used++] = (char)character;
        character = fgetc(file);
    }
    token[used] = '\0';
    return 1;
}

static int show_qr_image(const char *path)
{
    FILE *file = fopen(path, "rb");
    char token[64];
    int binary;
    int width;
    int height;
    int scale;
    int origin_x;
    int origin_y;
    int x;
    int y;
    Uint32 black;
    Uint32 white;

    if (!file)
        return -1;
    if (!pbm_token(file, token, sizeof(token))) {
        fclose(file);
        return -1;
    }
    binary = !strcmp(token, "P4");
    if (!binary && strcmp(token, "P1")) {
        fclose(file);
        return -1;
    }
    if (!pbm_token(file, token, sizeof(token))) {
        fclose(file);
        return -1;
    }
    width = atoi(token);
    if (!pbm_token(file, token, sizeof(token))) {
        fclose(file);
        return -1;
    }
    height = atoi(token);
    if (width <= 0 || height <= 0 || width > 512 || height > 512) {
        fclose(file);
        return -1;
    }

    scale = (screen->w - 16) / width;
    if ((screen->h - 36) / height < scale)
        scale = (screen->h - 36) / height;
    if (scale < 1)
        scale = 1;
    origin_x = (screen->w - width * scale) / 2;
    origin_y = 28 + (screen->h - 28 - height * scale) / 2;
    black = SDL_MapRGB(screen->format, 0, 0, 0);
    white = SDL_MapRGB(screen->format, 255, 255, 255);

    SDL_FillRect(screen, NULL, white);
    draw_text(8, 7, 2, "PAIR THIS RG NANO", black);

    if (binary) {
        int bytes_per_row = (width + 7) / 8;
        for (y = 0; y < height; ++y) {
            for (x = 0; x < bytes_per_row; ++x) {
                int byte = fgetc(file);
                int bit;
                if (byte == EOF) {
                    fclose(file);
                    return -1;
                }
                for (bit = 0; bit < 8; ++bit) {
                    int px = x * 8 + bit;
                    if (px < width && (byte & (1 << (7 - bit)))) {
                        fill_rect(
                            origin_x + px * scale,
                            origin_y + y * scale,
                            scale,
                            scale,
                            black
                        );
                    }
                }
            }
        }
    } else {
        for (y = 0; y < height; ++y) {
            for (x = 0; x < width; ++x) {
                if (!pbm_token(file, token, sizeof(token))) {
                    fclose(file);
                    return -1;
                }
                if (token[0] == '1') {
                    fill_rect(
                        origin_x + x * scale,
                        origin_y + y * scale,
                        scale,
                        scale,
                        black
                    );
                }
            }
        }
    }
    fclose(file);
    SDL_Flip(screen);
    wait_for_dismiss();
    return 0;
}

static const char *action_for(int selection)
{
    switch (selection) {
    case 0: return "toggle-service";
    case 1: return "ticket-qr";
    case 2: return "import-tickets";
    case 3: return "cycle-peer";
    case 4: return "snapshot-last";
    case 5: return "send-last";
    case 6: return "toggle-autosync";
    case 7: return "install-next";
    case 8: return "cycle-usb";
    case 9: return "sftp";
    case 10: return "netplay";
    case 11: return "diagnostics";
    default: return NULL;
    }
}

static void perform_action(int selection)
{
    const char *action = action_for(selection);
    char command[512];
    int status;
    if (!action)
        return;

    snprintf(
        command,
        sizeof(command),
        "/usr/bin/funkey-iroh-ui-action %s >/tmp/funkey-iroh-ui.out 2>&1",
        action
    );
    status = system(command);

    if (selection == 1 && status == 0
        && show_qr_image("/tmp/funkey-iroh-ticket.pbm") == 0) {
        return;
    }
    show_message(status == 0 ? "DONE" : "ACTION FAILED", "/tmp/funkey-iroh-ui.out");
}

static int initialize_video(int self_test)
{
    const SDL_VideoInfo *info;
    int width = SCREEN_FALLBACK;
    int height = SCREEN_FALLBACK;

    if (self_test)
        setenv("SDL_VIDEODRIVER", "dummy", 1);
    if (SDL_Init(SDL_INIT_VIDEO) != 0) {
        fprintf(stderr, "funkey-iroh-ui: SDL_Init failed: %s\n", SDL_GetError());
        return -1;
    }

    info = SDL_GetVideoInfo();
    if (info && info->current_w > 0 && info->current_h > 0) {
        width = info->current_w;
        height = info->current_h;
    }
    screen = SDL_SetVideoMode(
        width,
        height,
        16,
        self_test ? SDL_SWSURFACE : (SDL_SWSURFACE | SDL_FULLSCREEN)
    );
    if (!screen && (width != SCREEN_FALLBACK || height != SCREEN_FALLBACK)) {
        screen = SDL_SetVideoMode(
            SCREEN_FALLBACK,
            SCREEN_FALLBACK,
            16,
            self_test ? SDL_SWSURFACE : (SDL_SWSURFACE | SDL_FULLSCREEN)
        );
    }
    if (!screen) {
        fprintf(stderr, "funkey-iroh-ui: video mode failed: %s\n", SDL_GetError());
        SDL_Quit();
        return -1;
    }

    background = SDL_MapRGB(screen->format, 4, 16, 12);
    foreground = SDL_MapRGB(screen->format, 160, 255, 190);
    selected_bg = SDL_MapRGB(screen->format, 160, 255, 190);
    selected_fg = SDL_MapRGB(screen->format, 4, 16, 12);
    muted = SDL_MapRGB(screen->format, 85, 155, 105);
    SDL_EnableKeyRepeat(250, 80);
    SDL_ShowCursor(SDL_DISABLE);
    return 0;
}

int main(int argc, char **argv)
{
    struct summary value;
    SDL_Event event;
    int selection = 0;
    int running = 1;
    int self_test = argc > 1 && !strcmp(argv[1], "--self-test");

    if (initialize_video(self_test) != 0)
        return 1;

    summary_defaults(&value);
    if (self_test) {
        copy_value(value.service, sizeof(value.service), "RUNNING");
        copy_value(value.default_peer, sizeof(value.default_peer), "POCKET");
        copy_value(value.last_game, sizeof(value.last_game), "TEST GAME.GBC");
        render_menu(0, &value);
        SDL_Quit();
        puts("funkey-iroh-ui self-test: PASS");
        return 0;
    }

    load_summary(&value);
    render_menu(selection, &value);

    while (running) {
        if (!SDL_WaitEvent(&event))
            continue;
        if (event.type == SDL_QUIT) {
            running = 0;
        } else if (event.type == SDL_KEYDOWN) {
            SDLKey key = event.key.keysym.sym;
            if (is_up(key)) {
                selection = (selection + MENU_COUNT - 1) % MENU_COUNT;
            } else if (is_down(key)) {
                selection = (selection + 1) % MENU_COUNT;
            } else if (is_back(key)) {
                running = 0;
            } else if (is_accept(key)) {
                if (selection == MENU_COUNT - 1) {
                    running = 0;
                } else {
                    perform_action(selection);
                    load_summary(&value);
                }
            }
            if (running)
                render_menu(selection, &value);
        }
    }

    SDL_Quit();
    return 0;
}
