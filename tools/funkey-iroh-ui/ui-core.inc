static void copy_text(char *destination, size_t size, const char *source)
{
    if (size == 0) {
        return;
    }
    if (source == NULL) {
        source = "";
    }
    snprintf(destination, size, "%s", source);
}

static void append_text(char *destination, size_t size, const char *source)
{
    size_t used = strlen(destination);
    if (used >= size - 1) {
        return;
    }
    snprintf(destination + used, size - used, "%s", source == NULL ? "" : source);
}

static int file_exists(const char *path)
{
    struct stat st;
    return path != NULL && stat(path, &st) == 0;
}

static int write_atomic(const char *path, const char *contents)
{
    char temporary[PATH_MAX];
    FILE *file;

    snprintf(temporary, sizeof(temporary), "%s.tmp.%ld", path, (long)getpid());
    file = fopen(temporary, "w");
    if (file == NULL) {
        return -1;
    }
    if (fputs(contents, file) == EOF || fflush(file) != 0 || fsync(fileno(file)) != 0) {
        fclose(file);
        unlink(temporary);
        return -1;
    }
    if (fclose(file) != 0 || rename(temporary, path) != 0) {
        unlink(temporary);
        return -1;
    }
    return 0;
}

static int command_capture(char *const argv[], char *output, size_t output_size)
{
    int pipefd[2];
    pid_t child;
    int status = 127;
    size_t used = 0;

    if (output_size > 0) {
        output[0] = '\0';
    }
    if (pipe(pipefd) != 0) {
        return 127;
    }

    child = fork();
    if (child == 0) {
        int nullfd;
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        dup2(pipefd[1], STDERR_FILENO);
        close(pipefd[1]);
        nullfd = open("/dev/null", O_RDONLY);
        if (nullfd >= 0) {
            dup2(nullfd, STDIN_FILENO);
            close(nullfd);
        }
        execvp(argv[0], argv);
        dprintf(STDERR_FILENO, "%s: %s\n", argv[0], strerror(errno));
        _exit(127);
    }
    close(pipefd[1]);

    if (child < 0) {
        close(pipefd[0]);
        return 127;
    }

    for (;;) {
        char buffer[1024];
        ssize_t count = read(pipefd[0], buffer, sizeof(buffer));
        if (count == 0) {
            break;
        }
        if (count < 0) {
            if (errno == EINTR) {
                continue;
            }
            break;
        }
        if (output_size > 1 && used < output_size - 1) {
            size_t available = output_size - 1 - used;
            size_t copy = (size_t)count < available ? (size_t)count : available;
            memcpy(output + used, buffer, copy);
            used += copy;
            output[used] = '\0';
        }
    }
    close(pipefd[0]);

    while (waitpid(child, &status, 0) < 0 && errno == EINTR) {
    }
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status)) {
        return 128 + WTERMSIG(status);
    }
    return 127;
}

static int command_status(char *const argv[])
{
    char ignored[1024];
    return command_capture(argv, ignored, sizeof(ignored));
}

static void trim(char *text)
{
    size_t length;
    char *start = text;

    while (*start != '\0' && isspace((unsigned char)*start)) {
        start++;
    }
    if (start != text) {
        memmove(text, start, strlen(start) + 1);
    }

    length = strlen(text);
    while (length > 0 && isspace((unsigned char)text[length - 1])) {
        text[--length] = '\0';
    }
}

static SDL_Color sdl_color(Uint32 pixel)
{
    SDL_Color color;
    SDL_GetRGB(pixel, ui.screen->format, &color.r, &color.g, &color.b);
    color.unused = 255;
    return color;
}

static void fill_rect(int x, int y, int width, int height, Uint32 color)
{
    SDL_Rect rectangle;
    rectangle.x = (Sint16)x;
    rectangle.y = (Sint16)y;
    rectangle.w = (Uint16)(width < 0 ? 0 : width);
    rectangle.h = (Uint16)(height < 0 ? 0 : height);
    SDL_FillRect(ui.screen, &rectangle, color);
}

static int text_width(TTF_Font *font, const char *text)
{
    int width = 0;
    int height = 0;
    if (font != NULL && text != NULL) {
        TTF_SizeUTF8(font, text, &width, &height);
    }
    return width;
}

static void draw_text(TTF_Font *font, const char *text, int x, int y, Uint32 color)
{
    SDL_Surface *surface;
    SDL_Rect destination;
    SDL_Color foreground;

    if (font == NULL || text == NULL || *text == '\0') {
        return;
    }
    foreground = sdl_color(color);
    surface = TTF_RenderUTF8_Blended(font, text, foreground);
    if (surface == NULL) {
        return;
    }
    destination.x = (Sint16)x;
    destination.y = (Sint16)y;
    destination.w = 0;
    destination.h = 0;
    SDL_BlitSurface(surface, NULL, ui.screen, &destination);
    SDL_FreeSurface(surface);
}

static void ellipsize(TTF_Font *font, const char *text, int maximum, char *output, size_t size)
{
    size_t length;

    copy_text(output, size, text);
    if (text_width(font, output) <= maximum) {
        return;
    }
    length = strlen(output);
    while (length > 3 && text_width(font, output) > maximum) {
        output[--length] = '\0';
    }
    while (length > 0 && text_width(font, output) + text_width(font, "...") > maximum) {
        output[--length] = '\0';
    }
    append_text(output, size, "...");
}

static int draw_wrapped(TTF_Font *font, const char *text, int x, int y,
                        int maximum_width, int line_height, int maximum_lines,
                        Uint32 color)
{
    char working[MAX_OUTPUT];
    char line[1024] = "";
    char *saveptr = NULL;
    char *token;
    int lines = 0;

    copy_text(working, sizeof(working), text == NULL ? "" : text);
    token = strtok_r(working, " \t\r\n", &saveptr);
    while (token != NULL && lines < maximum_lines) {
        char candidate[1024];
        if (line[0] == '\0') {
            copy_text(candidate, sizeof(candidate), token);
        } else {
            snprintf(candidate, sizeof(candidate), "%s %s", line, token);
        }

        if (line[0] != '\0' && text_width(font, candidate) > maximum_width) {
            draw_text(font, line, x, y + lines * line_height, color);
            lines++;
            copy_text(line, sizeof(line), token);
        } else {
            copy_text(line, sizeof(line), candidate);
        }
        token = strtok_r(NULL, " \t\r\n", &saveptr);
    }
    if (line[0] != '\0' && lines < maximum_lines) {
        char fitted[1024];
        ellipsize(font, line, maximum_width, fitted, sizeof(fitted));
        draw_text(font, fitted, x, y + lines * line_height, color);
        lines++;
    }
    return lines;
}

static void draw_header(const char *title, const char *subtitle)
{
    fill_rect(0, 0, ui.width, ui.height, ui.background);
    fill_rect(0, 0, ui.width, 37, ui.panel);
    draw_text(ui.title_font, title, 10, 4, ui.foreground);
    if (subtitle != NULL && *subtitle != '\0') {
        char fitted[256];
        ellipsize(ui.small_font, subtitle, ui.width - 20, fitted, sizeof(fitted));
        draw_text(ui.small_font, fitted, 10, 25, ui.muted);
    }
}

static void draw_footer(const char *text)
{
    fill_rect(0, ui.height - 20, ui.width, 20, ui.panel);
    draw_text(ui.small_font, text, 8, ui.height - 18, ui.muted);
}

static UiKey wait_key(void)
{
    SDL_Event event;
    for (;;) {
        if (!SDL_WaitEvent(&event)) {
            continue;
        }
        if (event.type == SDL_QUIT) {
            return KEY_QUIT;
        }
        if (event.type != SDL_KEYDOWN) {
            continue;
        }
        switch (event.key.keysym.sym) {
        case SDLK_u:
        case SDLK_UP:
            return KEY_UP;
        case SDLK_d:
        case SDLK_DOWN:
            return KEY_DOWN;
        case SDLK_l:
        case SDLK_LEFT:
            return KEY_LEFT;
        case SDLK_r:
        case SDLK_RIGHT:
            return KEY_RIGHT;
        case SDLK_s:
        case SDLK_RETURN:
        case SDLK_SPACE:
            return KEY_SELECT;
        case SDLK_q:
        case SDLK_ESCAPE:
        case SDLK_BACKSPACE:
            return KEY_BACK;
        default:
            break;
        }
    }
}

static int first_enabled(MenuItem *items, int count)
{
    int index;
    for (index = 0; index < count; index++) {
        if (items[index].enabled) {
            return index;
        }
    }
    return 0;
}

static int next_enabled(MenuItem *items, int count, int current, int direction)
{
    int attempts;
    int next = current;

    for (attempts = 0; attempts < count; attempts++) {
        next += direction;
        if (next < 0) {
            next = count - 1;
        } else if (next >= count) {
            next = 0;
        }
        if (items[next].enabled) {
            return next;
        }
    }
    return current;
}

/*
 * Return values:
 *   -1: back
 *   0..count-1: select
 *   1000+index: right
 *   2000+index: left
 */
static int run_menu(const char *title, const char *subtitle,
                    MenuItem *items, int count, int initial, int lateral)
{
    int selected = initial;
    int row_height = ui.height >= 300 ? 43 : 34;
    int top = 42;
    int footer = 22;
    int visible = (ui.height - top - footer) / row_height;

    if (count <= 0) {
        return -1;
    }
    if (selected < 0 || selected >= count || !items[selected].enabled) {
        selected = first_enabled(items, count);
    }

    for (;;) {
        int start = 0;
        int index;

        if (selected >= visible) {
            start = selected - visible + 1;
        }
        if (start + visible > count) {
            start = count - visible;
        }
        if (start < 0) {
            start = 0;
        }

        draw_header(title, subtitle);
        for (index = 0; index < visible && start + index < count; index++) {
            int item_index = start + index;
            int y = top + index * row_height;
            Uint32 row_color = item_index == selected ? ui.selected : ui.background;
            Uint32 text_color = items[item_index].enabled ? ui.foreground : ui.muted;
            char label[MAX_LABEL + 8];
            char fitted[MAX_LABEL + 8];

            fill_rect(5, y, ui.width - 10, row_height - 2, row_color);
            if (items[item_index].checked >= 0) {
                snprintf(label, sizeof(label), "%s %s",
                         items[item_index].checked ? "[x]" : "[ ]",
                         items[item_index].label);
            } else {
                copy_text(label, sizeof(label), items[item_index].label);
            }
            ellipsize(ui.body_font, label, ui.width - 22, fitted, sizeof(fitted));
            draw_text(ui.body_font, fitted, 11, y + 3, text_color);
            if (items[item_index].detail[0] != '\0' && row_height >= 34) {
                ellipsize(ui.small_font, items[item_index].detail,
                          ui.width - 22, fitted, sizeof(fitted));
                draw_text(ui.small_font, fitted, 12, y + row_height - 16,
                          items[item_index].enabled ? ui.muted : ui.warning);
            }
        }
        draw_footer(lateral ? "U/D move  S select  L/R actions  Q back"
                            : "U/D move  S select  Q back");
        SDL_Flip(ui.screen);

        switch (wait_key()) {
        case KEY_UP:
            selected = next_enabled(items, count, selected, -1);
            break;
        case KEY_DOWN:
            selected = next_enabled(items, count, selected, 1);
            break;
        case KEY_LEFT:
            if (lateral) {
                return 2000 + selected;
            }
            break;
        case KEY_RIGHT:
            if (lateral) {
                return 1000 + selected;
            }
            break;
        case KEY_SELECT:
            return selected;
        case KEY_BACK:
        case KEY_QUIT:
            return -1;
        default:
            break;
        }
    }
}

static void message_screen(const char *title, const char *text, Uint32 accent)
{
    int offset = 0;
    int line_height = TTF_FontHeight(ui.body_font) + 2;
    char working[MAX_OUTPUT];
    char *lines[MAX_ITEMS * 4];
    int count = 0;
    char *saveptr = NULL;
    char *line;

    copy_text(working, sizeof(working), text == NULL ? "" : text);
    line = strtok_r(working, "\n", &saveptr);
    while (line != NULL && count < (int)(sizeof(lines) / sizeof(lines[0]))) {
        lines[count++] = line;
        line = strtok_r(NULL, "\n", &saveptr);
    }
    if (count == 0) {
        lines[count++] = "";
    }

    for (;;) {
        int visible = (ui.height - 65) / line_height;
        int index;
        draw_header(title, "");
        fill_rect(6, 42, ui.width - 12, ui.height - 66, ui.panel);
        fill_rect(6, 42, 4, ui.height - 66, accent);
        for (index = 0; index < visible && offset + index < count; index++) {
            char fitted[1024];
            ellipsize(ui.small_font, lines[offset + index], ui.width - 28,
                      fitted, sizeof(fitted));
            draw_text(ui.small_font, fitted, 17, 47 + index * line_height,
                      ui.foreground);
        }
        draw_footer("U/D scroll  Q/S back");
        SDL_Flip(ui.screen);

        switch (wait_key()) {
        case KEY_UP:
            if (offset > 0) {
                offset--;
            }
            break;
        case KEY_DOWN:
            if (offset + visible < count) {
                offset++;
            }
            break;
        case KEY_SELECT:
        case KEY_BACK:
        case KEY_QUIT:
            return;
        default:
            break;
        }
    }
}

static int confirm_screen(const char *title, const char *question)
{
    MenuItem choices[2];
    memset(choices, 0, sizeof(choices));
    copy_text(choices[0].label, sizeof(choices[0].label), "No, go back");
    choices[0].enabled = 1;
    choices[0].checked = -1;
    copy_text(choices[1].label, sizeof(choices[1].label), "Yes");
    choices[1].enabled = 1;
    choices[1].checked = -1;
    return run_menu(title, question, choices, 2, 0, 0) == 1;
}

static int service_running(void)
{
    char *const argv[] = {"/usr/bin/funkey-iroh-service", "status", NULL};
    return command_status(argv) == 0;
}

static int marker_enabled(const char *kind)
{
    char output[128];
    char *const argv[] = {
        "/usr/bin/funkey-iroh-sync-peer", (char *)kind, "status", NULL
    };
    return command_capture(argv, output, sizeof(output)) == 0;
}

static void toggle_marker(const char *kind)
{
    char output[256];
    char *const argv[] = {
        "/usr/bin/funkey-iroh-sync-peer", (char *)kind, "toggle", NULL
    };
    int status = command_capture(argv, output, sizeof(output));
    message_screen(status == 0 ? "Updated" : "Could not update",
                   output, status == 0 ? ui.success : ui.warning);
}

static int load_peers(Peer *peers, int maximum)
{
    char output[MAX_OUTPUT];
    char *const argv[] = {"/usr/bin/funkey-iroh-sync-peer", "list", NULL};
    char *saveptr = NULL;
    char *line;
    int count = 0;

    if (command_capture(argv, output, sizeof(output)) != 0) {
        return 0;
    }
    line = strtok_r(output, "\n", &saveptr);
    while (line != NULL && count < maximum) {
        char *fieldptr = NULL;
        char *enabled = strtok_r(line, "\t", &fieldptr);
        char *name = strtok_r(NULL, "\t", &fieldptr);
        char *id = strtok_r(NULL, "\t", &fieldptr);
        if (enabled != NULL && name != NULL && id != NULL) {
            peers[count].sync = atoi(enabled) != 0;
            copy_text(peers[count].name, sizeof(peers[count].name), name);
            copy_text(peers[count].id, sizeof(peers[count].id), id);
            count++;
        }
        line = strtok_r(NULL, "\n", &saveptr);
    }
    return count;
}

static int load_inbox(InboxItem *items, int maximum)
{
    char output[MAX_OUTPUT];
    char *const argv[] = {"/usr/bin/funkey-iroh-inbox", "list", NULL};
    char *saveptr = NULL;
    char *line;
    int count = 0;

    if (command_capture(argv, output, sizeof(output)) != 0) {
        return 0;
    }
    line = strtok_r(output, "\n", &saveptr);
    while (line != NULL && count < maximum) {
        char *fieldptr = NULL;
        char *path = strtok_r(line, "\t", &fieldptr);
        char *peer = strtok_r(NULL, "\t", &fieldptr);
        char *system = strtok_r(NULL, "\t", &fieldptr);
        char *game = strtok_r(NULL, "\t", &fieldptr);
        char *filename = strtok_r(NULL, "\t", &fieldptr);
        char *size = strtok_r(NULL, "\t", &fieldptr);
        char *target = strtok_r(NULL, "\t", &fieldptr);
        if (path && peer && system && game && filename && size && target) {
            copy_text(items[count].path, sizeof(items[count].path), path);
            copy_text(items[count].peer, sizeof(items[count].peer), peer);
            copy_text(items[count].system, sizeof(items[count].system), system);
            copy_text(items[count].game, sizeof(items[count].game), game);
            copy_text(items[count].filename, sizeof(items[count].filename), filename);
            copy_text(items[count].target, sizeof(items[count].target), target);
            items[count].size = strtoull(size, NULL, 10);
            count++;
        }
        line = strtok_r(NULL, "\n", &saveptr);
    }
    return count;
}

static int count_outbox(void)
{
    char output[64];
    char *const argv[] = {"/usr/bin/funkey-iroh-outbox", "count", NULL};
    if (command_capture(argv, output, sizeof(output)) != 0) {
        return 0;
    }
    return atoi(output);
}

static void service_toggle(void)
{
    char output[MAX_OUTPUT];
    char *const enable[] = {"/usr/bin/funkey-iroh-service", "enable", NULL};
    char *const disable[] = {"/usr/bin/funkey-iroh-service", "disable", NULL};
    int running = service_running();
    int status = command_capture(running ? disable : enable, output, sizeof(output));
    if (output[0] == '\0') {
        copy_text(output, sizeof(output),
                  status == 0 ? "The Iroh service setting was updated."
                              : "The Iroh service could not be updated.");
    }
    message_screen(status == 0 ? "Iroh service" : "Service error",
                   output, status == 0 ? ui.success : ui.warning);
}
