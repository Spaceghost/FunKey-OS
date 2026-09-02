typedef struct {
    int width;
    int height;
    unsigned char *bits;
} Pbm;

static void pbm_free(Pbm *image)
{
    free(image->bits);
    image->bits = NULL;
    image->width = 0;
    image->height = 0;
}

static int pbm_next_token(FILE *file, char *token, size_t size)
{
    int character;
    size_t used = 0;

    for (;;) {
        character = fgetc(file);
        if (character == '#') {
            while (character != '\n' && character != EOF) {
                character = fgetc(file);
            }
            continue;
        }
        if (character == EOF) {
            return 0;
        }
        if (!isspace(character)) {
            break;
        }
    }

    do {
        if (used + 1 < size) {
            token[used++] = (char)character;
        }
        character = fgetc(file);
    } while (character != EOF && !isspace(character));
    token[used] = '\0';
    return used > 0;
}

static int pbm_load(const char *path, Pbm *image)
{
    FILE *file = fopen(path, "rb");
    char magic[8];
    char token[64];
    int binary;
    int x, y;

    memset(image, 0, sizeof(*image));
    if (file == NULL || !pbm_next_token(file, magic, sizeof(magic))) {
        if (file) fclose(file);
        return -1;
    }
    binary = strcmp(magic, "P4") == 0;
    if (!binary && strcmp(magic, "P1") != 0) {
        fclose(file);
        return -1;
    }
    if (!pbm_next_token(file, token, sizeof(token))) {
        fclose(file);
        return -1;
    }
    image->width = atoi(token);
    if (!pbm_next_token(file, token, sizeof(token))) {
        fclose(file);
        return -1;
    }
    image->height = atoi(token);
    if (image->width <= 0 || image->height <= 0 ||
        image->width > 2048 || image->height > 2048) {
        fclose(file);
        return -1;
    }
    image->bits = calloc((size_t)image->width * (size_t)image->height, 1);
    if (image->bits == NULL) {
        fclose(file);
        return -1;
    }

    if (binary) {
        int bytes_per_row = (image->width + 7) / 8;
        for (y = 0; y < image->height; y++) {
            for (x = 0; x < bytes_per_row; x++) {
                int value = fgetc(file);
                int bit;
                if (value == EOF) {
                    pbm_free(image);
                    fclose(file);
                    return -1;
                }
                for (bit = 0; bit < 8; bit++) {
                    int px = x * 8 + bit;
                    if (px < image->width) {
                        image->bits[y * image->width + px] =
                            (unsigned char)((value >> (7 - bit)) & 1);
                    }
                }
            }
        }
    } else {
        for (y = 0; y < image->height; y++) {
            for (x = 0; x < image->width; x++) {
                if (!pbm_next_token(file, token, sizeof(token))) {
                    pbm_free(image);
                    fclose(file);
                    return -1;
                }
                image->bits[y * image->width + x] = token[0] == '1';
            }
        }
    }
    fclose(file);
    return 0;
}

static int generate_qr(const char *ticket, Pbm *image)
{
    const char *path = "/tmp/funkey-iroh-ticket.pbm";
    char *const argv[] = {
        "qrencode", "-t", "PBM", "-m", "1", "-o", (char *)path,
        (char *)ticket, NULL
    };
    unlink(path);
    if (command_status(argv) != 0 || pbm_load(path, image) != 0) {
        unlink(path);
        return -1;
    }
    unlink(path);
    return 0;
}

static void draw_qr(Pbm *image, int x, int y, int maximum)
{
    int scale;
    int draw_width;
    int offset_x;
    int offset_y;
    int row, column;

    if (image->bits == NULL || image->width <= 0 || image->height <= 0) {
        return;
    }
    scale = maximum / (image->width > image->height ? image->width : image->height);
    if (scale < 1) {
        scale = 1;
    }
    draw_width = image->width * scale;
    offset_x = x + (maximum - draw_width) / 2;
    offset_y = y + (maximum - image->height * scale) / 2;
    fill_rect(x, y, maximum, maximum, ui.foreground);
    for (row = 0; row < image->height; row++) {
        for (column = 0; column < image->width; column++) {
            Uint32 color = image->bits[row * image->width + column]
                               ? ui.background : ui.foreground;
            fill_rect(offset_x + column * scale, offset_y + row * scale,
                      scale, scale, color);
        }
    }
}

static void pairing_screen(void)
{
    char output[MAX_OUTPUT];
    char *const argv[] = {"/usr/bin/funkey-iroh-pairing", "export", NULL};
    int status = command_capture(argv, output, sizeof(output));
    char *fieldptr = NULL;
    char *name;
    char *path;
    char *ticket;
    Pbm qr;

    if (status != 0) {
        message_screen("Pairing failed", output, ui.warning);
        return;
    }
    trim(output);
    name = strtok_r(output, "\t", &fieldptr);
    path = strtok_r(NULL, "\t", &fieldptr);
    ticket = strtok_r(NULL, "\t", &fieldptr);
    if (name == NULL || path == NULL || ticket == NULL) {
        message_screen("Pairing failed", "The pairing helper returned an invalid card.", ui.warning);
        return;
    }

    memset(&qr, 0, sizeof(qr));
    generate_qr(ticket, &qr);
    for (;;) {
        int qr_size = ui.height - 95;
        char path_line[PATH_MAX + 32];
        char short_ticket[96];

        if (qr_size > ui.width - 20) {
            qr_size = ui.width - 20;
        }
        if (qr_size < 80) {
            qr_size = 80;
        }

        draw_header("Pair this device", name);
        if (qr.bits != NULL) {
            draw_qr(&qr, 8, 43, qr_size);
            snprintf(path_line, sizeof(path_line), "Card: %s", path);
            ellipsize(ui.small_font, path_line, ui.width - qr_size - 26,
                      short_ticket, sizeof(short_ticket));
            draw_text(ui.small_font, short_ticket, qr_size + 16, 51, ui.muted);
            draw_wrapped(ui.small_font,
                         "Scan this QR from a companion or copy the .pair card to the other device's Pairing folder.",
                         qr_size + 16, 72, ui.width - qr_size - 24,
                         TTF_FontHeight(ui.small_font) + 2, 8, ui.foreground);
        } else {
            draw_wrapped(ui.body_font,
                         "QR generation is unavailable. Copy this pairing card to the other device:",
                         12, 49, ui.width - 24,
                         TTF_FontHeight(ui.body_font) + 3, 5, ui.foreground);
            draw_wrapped(ui.small_font, path, 12, 130, ui.width - 24,
                         TTF_FontHeight(ui.small_font) + 2, 4, ui.success);
            draw_wrapped(ui.small_font, ticket, 12, 178, ui.width - 24,
                         TTF_FontHeight(ui.small_font) + 2, 5, ui.muted);
        }
        draw_footer("S refresh pairing card  Q back");
        SDL_Flip(ui.screen);

        switch (wait_key()) {
        case KEY_SELECT:
            pbm_free(&qr);
            command_capture(argv, output, sizeof(output));
            message_screen("Pairing card refreshed", path, ui.success);
            return;
        case KEY_BACK:
        case KEY_QUIT:
            pbm_free(&qr);
            return;
        default:
            break;
        }
    }
}
