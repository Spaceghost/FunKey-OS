static void import_pairing_cards(void)
{
    char output[MAX_OUTPUT];
    char *const argv[] = {"/usr/bin/funkey-iroh-pairing", "import", NULL};
    int status = command_capture(argv, output, sizeof(output));
    message_screen(status == 0 ? "Pairing import" : "Pairing error",
                   output[0] ? output : "No pairing cards were found.",
                   status == 0 ? ui.success : ui.warning);
}

static void device_name_screen(void)
{
    char name[128] = "unnamed";
    char output[MAX_OUTPUT];
    char *const read_name[] = {"/usr/bin/funkey-iroh-pairing", "name", NULL};
    char *const import_name[] = {"/usr/bin/funkey-iroh-pairing", "import-name", NULL};
    MenuItem menu[2];
    int choice;

    if (command_capture(read_name, name, sizeof(name)) != 0) {
        copy_text(name, sizeof(name), "unnamed");
    }
    trim(name);

    memset(menu, 0, sizeof(menu));
    copy_text(menu[0].label, sizeof(menu[0].label), "Import device-name.txt");
    copy_text(menu[0].detail, sizeof(menu[0].detail),
              "Read /mnt/FunKey/Iroh/device-name.txt from USB storage");
    menu[0].enabled = 1;
    menu[0].checked = -1;
    copy_text(menu[1].label, sizeof(menu[1].label), "Back");
    menu[1].enabled = 1;
    menu[1].checked = -1;

    choice = run_menu("Device name", name, menu, 2, 0, 0);
    if (choice == 0) {
        int status = command_capture(import_name, output, sizeof(output));
        message_screen(status == 0 ? "Device renamed" : "Rename failed",
                       output, status == 0 ? ui.success : ui.warning);
    }
}

static void peers_screen(void)
{
    int selected = 0;
    for (;;) {
        Peer peers[MAX_ITEMS];
        MenuItem menu[MAX_ITEMS];
        int count = load_peers(peers, MAX_ITEMS);
        int index;
        int choice;

        if (count == 0) {
            message_screen("Peers",
                           "No peers are paired yet.\n\nOpen Pair this device, exchange .pair cards, then import the other card.",
                           ui.muted);
            return;
        }
        memset(menu, 0, sizeof(menu));
        for (index = 0; index < count; index++) {
            copy_text(menu[index].label, sizeof(menu[index].label), peers[index].name);
            snprintf(menu[index].detail, sizeof(menu[index].detail),
                     "%.40s  |  post-game sync %s",
                     peers[index].id, peers[index].sync ? "on" : "off");
            menu[index].enabled = 1;
            menu[index].checked = peers[index].sync;
        }
        if (selected >= count) {
            selected = count - 1;
        }

        choice = run_menu("Peers & save sync",
                          "S toggles automatic post-game sync; R removes",
                          menu, count, selected, 1);
        if (choice < 0) {
            return;
        }
        if (choice >= 1000 && choice < 2000) {
            int remove_index = choice - 1000;
            char output[MAX_OUTPUT];
            char *const remove_peer[] = {
                "/usr/bin/funkey-iroh", "peer", "remove",
                peers[remove_index].name, NULL
            };
            char *const disable_sync[] = {
                "/usr/bin/funkey-iroh-sync-peer", "disable",
                peers[remove_index].name, NULL
            };
            selected = remove_index;
            if (confirm_screen("Remove peer?", peers[remove_index].name)) {
                command_status(disable_sync);
                if (command_capture(remove_peer, output, sizeof(output)) != 0) {
                    message_screen("Could not remove peer", output, ui.warning);
                }
            }
            continue;
        }
        if (choice >= 2000) {
            selected = choice - 2000;
            continue;
        }

        selected = choice;
        {
            char output[1024];
            char *const toggle[] = {
                "/usr/bin/funkey-iroh-sync-peer", "toggle",
                peers[choice].name, NULL
            };
            int status = command_capture(toggle, output, sizeof(output));
            if (status != 0) {
                message_screen("Sync setting failed", output, ui.warning);
            }
        }
    }
}

static void inbox_detail(InboxItem *item)
{
    for (;;) {
        MenuItem menu[3];
        char subtitle[256];
        int choice;
        memset(menu, 0, sizeof(menu));

        copy_text(menu[0].label, sizeof(menu[0].label), "Install safely");
        snprintf(menu[0].detail, sizeof(menu[0].detail),
                 "%s", strcmp(item->target, "-") == 0
                           ? "Play this game locally once to register its save path"
                           : "Back up local progress, then install this copy");
        menu[0].enabled = strcmp(item->target, "-") != 0;
        menu[0].checked = -1;

        copy_text(menu[1].label, sizeof(menu[1].label), "Archive incoming");
        copy_text(menu[1].detail, sizeof(menu[1].detail),
                  "Keep it without changing the live emulator save");
        menu[1].enabled = 1;
        menu[1].checked = -1;

        copy_text(menu[2].label, sizeof(menu[2].label), "Back");
        menu[2].enabled = 1;
        menu[2].checked = -1;

        snprintf(subtitle, sizeof(subtitle), "%s from %s (%llu bytes)",
                 item->filename, item->peer, item->size);
        choice = run_menu(item->game, subtitle, menu, 3, 0, 0);
        if (choice < 0 || choice == 2) {
            return;
        }
        if (choice == 0) {
            char output[MAX_OUTPUT];
            char *const install[] = {
                "/usr/bin/funkey-iroh-inbox", "install", item->path, NULL
            };
            if (!confirm_screen("Install received save?",
                                "The current local save will be backed up first.")) {
                continue;
            }
            int status = command_capture(install, output, sizeof(output));
            message_screen(status == 0 ? "Save installed" : "Install failed",
                           output, status == 0 ? ui.success : ui.warning);
            return;
        }
        if (choice == 1) {
            char output[MAX_OUTPUT];
            char *const archive[] = {
                "/usr/bin/funkey-iroh-inbox", "archive", item->path, NULL
            };
            int status = command_capture(archive, output, sizeof(output));
            message_screen(status == 0 ? "Incoming save archived" : "Archive failed",
                           output, status == 0 ? ui.success : ui.warning);
            return;
        }
    }
}

static void inbox_screen(void)
{
    int selected = 0;
    for (;;) {
        InboxItem inbox[MAX_ITEMS];
        MenuItem menu[MAX_ITEMS];
        int count = load_inbox(inbox, MAX_ITEMS);
        int index;
        int choice;

        if (count == 0) {
            message_screen("Received saves",
                           "The inbox is empty.\n\nIncoming files are hash-checked and held here until a safe local target is known.",
                           ui.success);
            return;
        }
        memset(menu, 0, sizeof(menu));
        for (index = 0; index < count; index++) {
            snprintf(menu[index].label, sizeof(menu[index].label),
                     "%s: %s", inbox[index].peer, inbox[index].game);
            snprintf(menu[index].detail, sizeof(menu[index].detail),
                     "%s  |  %s", inbox[index].filename,
                     strcmp(inbox[index].target, "-") == 0 ? "target unknown" : "ready");
            menu[index].enabled = 1;
            menu[index].checked = -1;
        }
        if (selected >= count) {
            selected = count - 1;
        }
        choice = run_menu("Received saves", "Nothing is overwritten silently",
                          menu, count, selected, 0);
        if (choice < 0) {
            return;
        }
        selected = choice;
        inbox_detail(&inbox[choice]);
    }
}

static void multiplayer_screen(void)
{
    char output[MAX_OUTPUT] = "";
    char adapters[MAX_OUTPUT] = "";
    char *const status_argv[] = {"/usr/bin/funkey-iroh-session", "status", NULL};
    char *const list_argv[] = {"/usr/bin/funkey-iroh-session", "list", NULL};
    command_capture(status_argv, output, sizeof(output));
    command_capture(list_argv, adapters, sizeof(adapters));

    if (adapters[0] == '\0') {
        append_text(output, sizeof(output),
                    "\n\nNo bundled emulator adapter is enabled yet.\n"
                    "The pre-game menu will show Solo and explain why multiplayer is unavailable. "
                    "As cores gain UDP or link-cable adapters, Host and Join appear automatically.");
    } else {
        append_text(output, sizeof(output), "\n\n-- adapters --\n");
        append_text(output, sizeof(output), adapters);
    }
    message_screen("Multiplayer", output, ui.foreground);
}

static void diagnostics_screen(void)
{
    char output[MAX_OUTPUT] = "";
    char temporary[MAX_OUTPUT];
    char *const status[] = {"/usr/bin/funkey-iroh-service", "status", NULL};
    char *const logs[] = {"/usr/bin/funkey-iroh-service", "logs", "12", NULL};
    char *const session[] = {"/usr/bin/funkey-iroh-session", "status", NULL};

    append_text(output, sizeof(output), "-- service --\n");
    command_capture(status, temporary, sizeof(temporary));
    append_text(output, sizeof(output), temporary);
    append_text(output, sizeof(output), "\n-- session --\n");
    command_capture(session, temporary, sizeof(temporary));
    append_text(output, sizeof(output), temporary);
    append_text(output, sizeof(output), "\n-- receiver log --\n");
    command_capture(logs, temporary, sizeof(temporary));
    append_text(output, sizeof(output), temporary);
    message_screen("Iroh diagnostics", output, ui.foreground);
}

static int main_ui(void)
{
    for (;;) {
        Peer peers[MAX_ITEMS];
        InboxItem inbox[MAX_ITEMS];
        MenuItem menu[11];
        int peer_count = load_peers(peers, MAX_ITEMS);
        int inbox_count = load_inbox(inbox, MAX_ITEMS);
        int queued = count_outbox();
        int running = service_running();
        int prompt = marker_enabled("prompt");
        int automatic = marker_enabled("auto");
        char subtitle[160];
        int choice;

        memset(menu, 0, sizeof(menu));
        snprintf(menu[0].label, sizeof(menu[0].label),
                 "Iroh receiver: %s", running ? "ON" : "OFF");
        copy_text(menu[0].detail, sizeof(menu[0].detail),
                  running ? "Encrypted paired save receiver is active"
                          : "Select to enable networking and the receiver");
        menu[0].enabled = 1;
        menu[0].checked = running;

        copy_text(menu[1].label, sizeof(menu[1].label), "Pair this device");
        copy_text(menu[1].detail, sizeof(menu[1].detail),
                  "Show QR and export a USB-friendly pairing card");
        menu[1].enabled = 1;
        menu[1].checked = -1;

        copy_text(menu[2].label, sizeof(menu[2].label), "Import pairing cards");
        copy_text(menu[2].detail, sizeof(menu[2].detail),
                  "Read .pair files copied into FunKey/Iroh/Pairing");
        menu[2].enabled = 1;
        menu[2].checked = -1;

        copy_text(menu[3].label, sizeof(menu[3].label), "Device name");
        copy_text(menu[3].detail, sizeof(menu[3].detail),
                  "Import a friendly name from USB storage");
        menu[3].enabled = 1;
        menu[3].checked = -1;

        copy_text(menu[4].label, sizeof(menu[4].label), "Peers & automatic sync");
        snprintf(menu[4].detail, sizeof(menu[4].detail),
                 "%d paired device%s", peer_count, peer_count == 1 ? "" : "s");
        menu[4].enabled = 1;
        menu[4].checked = -1;

        copy_text(menu[5].label, sizeof(menu[5].label), "Received saves");
        snprintf(menu[5].detail, sizeof(menu[5].detail),
                 "%d waiting; %d queued to send", inbox_count, queued);
        menu[5].enabled = 1;
        menu[5].checked = -1;

        copy_text(menu[6].label, sizeof(menu[6].label), "Multiplayer");
        copy_text(menu[6].detail, sizeof(menu[6].detail),
                  "Session status and emulator-adapter readiness");
        menu[6].enabled = 1;
        menu[6].checked = -1;

        copy_text(menu[7].label, sizeof(menu[7].label), "Ask before each game");
        copy_text(menu[7].detail, sizeof(menu[7].detail),
                  "Choose Solo / Host / Join before the emulator starts");
        menu[7].enabled = 1;
        menu[7].checked = prompt;

        copy_text(menu[8].label, sizeof(menu[8].label), "Safe automatic receive");
        copy_text(menu[8].detail, sizeof(menu[8].detail),
                  "Auto-install only when local progress matches its baseline");
        menu[8].enabled = 1;
        menu[8].checked = automatic;

        copy_text(menu[9].label, sizeof(menu[9].label), "Diagnostics");
        copy_text(menu[9].detail, sizeof(menu[9].detail),
                  "Receiver, sync queue, session, and recent logs");
        menu[9].enabled = 1;
        menu[9].checked = -1;

        copy_text(menu[10].label, sizeof(menu[10].label), "Exit");
        menu[10].enabled = 1;
        menu[10].checked = -1;

        snprintf(subtitle, sizeof(subtitle),
                 "%d peer%s  |  %d inbox  |  %d queued",
                 peer_count, peer_count == 1 ? "" : "s", inbox_count, queued);
        choice = run_menu("Iroh", subtitle, menu, 11, 0, 0);
        switch (choice) {
        case 0:
            service_toggle();
            break;
        case 1:
            pairing_screen();
            break;
        case 2:
            import_pairing_cards();
            break;
        case 3:
            device_name_screen();
            break;
        case 4:
            peers_screen();
            break;
        case 5:
            inbox_screen();
            break;
        case 6:
            multiplayer_screen();
            break;
        case 7:
            toggle_marker("prompt");
            break;
        case 8:
            toggle_marker("auto");
            break;
        case 9:
            diagnostics_screen();
            break;
        case 10:
        default:
            return 0;
        }
    }
}
