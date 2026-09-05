static int adapter_available(const char *system, char *description, size_t size)
{
    char *const argv[] = {
        "/usr/bin/funkey-iroh-session", "adapter", (char *)system, NULL
    };
    int status = command_capture(argv, description, size);
    trim(description);
    return status == 0 && description[0] != '\0';
}

static int choose_launch(const char *system, const char *game, const char *rom)
{
    Peer peers[MAX_ITEMS];
    MenuItem menu[MAX_ITEMS + 4];
    char adapter[MAX_OUTPUT];
    int peer_count = load_peers(peers, MAX_ITEMS);
    int has_adapter = adapter_available(system, adapter, sizeof(adapter));
    int running = service_running();
    int count = 0;
    int choice;
    int index;
    char subtitle[256];

    memset(menu, 0, sizeof(menu));

    copy_text(menu[count].label, sizeof(menu[count].label), "Play solo");
    copy_text(menu[count].detail, sizeof(menu[count].detail),
              "Start immediately; post-game save sync still runs");
    menu[count].enabled = 1;
    menu[count].checked = -1;
    count++;

    if (has_adapter) {
        copy_text(menu[count].label, sizeof(menu[count].label), "Host multiplayer");
        copy_text(menu[count].detail, sizeof(menu[count].detail),
                  running ? "Wait for a paired device over Iroh"
                          : "Enable Iroh in Settings first");
        menu[count].enabled = running;
        menu[count].checked = -1;
        count++;

        for (index = 0; index < peer_count && count < MAX_ITEMS + 2; index++) {
            snprintf(menu[count].label, sizeof(menu[count].label),
                     "Join %s", peers[index].name);
            copy_text(menu[count].detail, sizeof(menu[count].detail),
                      running ? "Connect encrypted, direct when possible"
                              : "Enable Iroh in Settings first");
            menu[count].enabled = running;
            menu[count].checked = -1;
            count++;
        }
    } else {
        copy_text(menu[count].label, sizeof(menu[count].label),
                  "Multiplayer adapter unavailable");
        copy_text(menu[count].detail, sizeof(menu[count].detail),
                  "This core has no UDP/link-cable bridge yet");
        menu[count].enabled = 0;
        menu[count].checked = -1;
        count++;
    }

    copy_text(menu[count].label, sizeof(menu[count].label), "Cancel");
    menu[count].enabled = 1;
    menu[count].checked = -1;
    count++;

    snprintf(subtitle, sizeof(subtitle), "%s%s%s",
             system, rom && *rom ? "  |  " : "",
             rom && *rom ? strrchr(rom, '/') ? strrchr(rom, '/') + 1 : rom : "");
    choice = run_menu(game && *game ? game : "Start game",
                      subtitle, menu, count, 0, 0);
    if (choice < 0 || choice == count - 1) {
        write_atomic(ACTION_FILE, "cancel\n");
        return 2;
    }
    if (choice == 0) {
        return write_atomic(ACTION_FILE, "solo\n") == 0 ? 0 : 1;
    }
    if (!has_adapter) {
        return write_atomic(ACTION_FILE, "solo\n") == 0 ? 0 : 1;
    }
    if (choice == 1) {
        return write_atomic(ACTION_FILE, "host\n") == 0 ? 0 : 1;
    }

    index = choice - 2;
    if (index >= 0 && index < peer_count) {
        char action[160];
        snprintf(action, sizeof(action), "join\t%s\n", peers[index].name);
        return write_atomic(ACTION_FILE, action) == 0 ? 0 : 1;
    }
    return 1;
}

static void close_ui_fonts(void)
{
    if (ui.title_font != NULL) {
        TTF_CloseFont(ui.title_font);
        ui.title_font = NULL;
    }
    if (ui.body_font != NULL) {
        TTF_CloseFont(ui.body_font);
        ui.body_font = NULL;
    }
    if (ui.small_font != NULL) {
        TTF_CloseFont(ui.small_font);
        ui.small_font = NULL;
    }
}

static int open_ui_fonts_from(const char *path)
{
    int large = ui.height >= 300;

    if (path == NULL || *path == '\0') {
        return -1;
    }

    ui.title_font = TTF_OpenFont(path, large ? 18 : 15);
    ui.body_font = TTF_OpenFont(path, large ? 15 : 13);
    ui.small_font = TTF_OpenFont(path, large ? 12 : 10);
    if (ui.title_font != NULL && ui.body_font != NULL && ui.small_font != NULL) {
        return 0;
    }

    close_ui_fonts();
    return -1;
}

static int open_ui_fonts(void)
{
    const char *override = getenv("FUNKEY_IROH_UI_FONT");
    const char *candidates[] = {
        FONT_PATH,
        FONT_PATH_FULL,
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        NULL
    };
    int index;

    if (override != NULL && *override != '\0' &&
        open_ui_fonts_from(override) == 0) {
        return 0;
    }

    for (index = 0; candidates[index] != NULL; index++) {
        if (open_ui_fonts_from(candidates[index]) == 0) {
            return 0;
        }
    }

    fprintf(stderr,
            "TTF_OpenFont: no usable UI font; set FUNKEY_IROH_UI_FONT (%s)\n",
            TTF_GetError());
    return -1;
}

static int initialize_ui(void)
{
    const char *driver = getenv("SDL_VIDEODRIVER");

    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK) != 0) {
        fprintf(stderr, "SDL_Init: %s\n", SDL_GetError());
        return -1;
    }
    if (TTF_Init() != 0) {
        fprintf(stderr, "TTF_Init: %s\n", TTF_GetError());
        SDL_Quit();
        return -1;
    }

    SDL_ShowCursor(SDL_DISABLE);
    ui.screen = SDL_SetVideoMode(0, 0, 16, SDL_HWSURFACE | SDL_TRIPLEBUF);
    if (ui.screen == NULL) {
        ui.screen = SDL_SetVideoMode(320, 240, 16, SDL_SWSURFACE);
    }
    if (ui.screen == NULL) {
        fprintf(stderr, "SDL_SetVideoMode: %s\n", SDL_GetError());
        TTF_Quit();
        SDL_Quit();
        return -1;
    }
    ui.width = ui.screen->w;
    ui.height = ui.screen->h;

    if (open_ui_fonts() != 0) {
        TTF_Quit();
        SDL_Quit();
        ui.screen = NULL;
        return -1;
    }

    ui.background = SDL_MapRGB(ui.screen->format, 7, 18, 12);
    ui.panel = SDL_MapRGB(ui.screen->format, 15, 37, 25);
    ui.selected = SDL_MapRGB(ui.screen->format, 27, 74, 47);
    ui.foreground = SDL_MapRGB(ui.screen->format, 211, 242, 218);
    ui.muted = SDL_MapRGB(ui.screen->format, 128, 170, 140);
    ui.warning = SDL_MapRGB(ui.screen->format, 242, 185, 92);
    ui.success = SDL_MapRGB(ui.screen->format, 123, 232, 157);

    SDL_EnableKeyRepeat(450, 85);
    (void)driver;
    return 0;
}

static void shutdown_ui(void)
{
    close_ui_fonts();
    TTF_Quit();
    SDL_Quit();
    ui.screen = NULL;
}

static int ui_self_test(void)
{
    int width = 0;
    int height = 0;

    if (TTF_SizeUTF8(ui.title_font, "Iroh UI", &width, &height) != 0 ||
        width <= 0 || height <= 0) {
        fprintf(stderr, "TTF_SizeUTF8: %s\n", TTF_GetError());
        return 1;
    }

    draw_header("Iroh UI", "native renderer self-test");
    draw_text(ui.body_font, "pair  sync  play", 12, 54, ui.foreground);
    draw_footer("self-test");
    if (SDL_Flip(ui.screen) != 0) {
        fprintf(stderr, "SDL_Flip: %s\n", SDL_GetError());
        return 1;
    }
    return 0;
}

static void usage(FILE *stream)
{
    fprintf(stream,
            "Usage:\n"
            "  funkey-iroh-ui\n"
            "  funkey-iroh-ui --launch SYSTEM GAME [ROM]\n"
            "  funkey-iroh-ui --session-status\n"
            "  funkey-iroh-ui --message TITLE TEXT\n"
            "  funkey-iroh-ui --self-test\n");
}

int main(int argc, char **argv)
{
    int status = 0;

    if (argc > 1 && (strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0)) {
        usage(stdout);
        return 0;
    }
    if (initialize_ui() != 0) {
        return 1;
    }

    if (argc == 1) {
        status = main_ui();
    } else if (strcmp(argv[1], "--launch") == 0) {
        if (argc < 4 || argc > 5) {
            usage(stderr);
            status = 64;
        } else {
            status = choose_launch(argv[2], argv[3], argc == 5 ? argv[4] : "");
        }
    } else if (strcmp(argv[1], "--session-status") == 0) {
        multiplayer_screen();
    } else if (strcmp(argv[1], "--message") == 0) {
        if (argc != 4) {
            usage(stderr);
            status = 64;
        } else {
            message_screen(argv[2], argv[3], ui.warning);
        }
    } else if (strcmp(argv[1], "--self-test") == 0) {
        if (argc != 2) {
            usage(stderr);
            status = 64;
        } else {
            status = ui_self_test();
        }
    } else {
        usage(stderr);
        status = 64;
    }

    shutdown_ui();
    return status;
}
