/* Temporary volume overlay on the four-digit display. The native clock owns
 * it again after two seconds; a native animation preempts the overlay early. */
enum { DISPLAY_LEDS = 34 };
struct display {
    int fd[DISPLAY_LEDS], active, brightness;
    unsigned char saved[DISPLAY_LEDS], last[DISPLAY_LEDS];
    int64_t until, check_at;
};
static void display_close(struct display *d) {
    if (!d->active)
        return;
    for (int i = 0; i < DISPLAY_LEDS; i++) {
        /* Restore only channels still carrying our last write. */
        if (ring_read(d->fd[i]) == d->last[i] && d->last[i] != d->saved[i])
            (void)ring_write(d->fd[i], d->saved[i]);
        close(d->fd[i]);
    }
    d->active = 0;
}
static int display_open(struct display *d) {
    static const char *extra[] = {"PLUS1", "PLUS2", "PLUS3", "PLUS4", "MINUS", "DEGREE"};
    int count = 0, brightness = 0;
    for (int i = 0; i < DISPLAY_LEDS; i++) {
        char path[128];
        size_t n = strlen(LED_ROOT);
        if (n + 40 > sizeof path)
            goto fail;
        memcpy(path, LED_ROOT, n);
        memcpy(path + n, "mini2:white:clock", 17);
        n += 17;
        if (i < 28) {
            path[n++] = (char)('A' + i % 7);
            path[n++] = (char)('1' + i / 7);
        } else {
            size_t len = strlen(extra[i - 28]);
            memcpy(path + n, extra[i - 28], len);
            n += len;
        }
        strcpy(path + n, "/brightness");
        int fd = open(path, O_RDWR | O_CLOEXEC);
        if (fd < 0)
            goto fail;
        d->fd[count++] = fd;
        int value = ring_read(fd);
        if (value < 0 || value > 170)
            goto fail;
        d->saved[i] = d->last[i] = (unsigned char)value;
        if (value > brightness)
            brightness = value;
    }
    d->brightness = brightness ? brightness : 22;
    d->active = 1;
    return 1;
fail:
    for (int i = 0; i < count; i++)
        close(d->fd[i]);
    return 0;
}
static int display_unchanged(struct display *d) {
    for (int i = 0; i < DISPLAY_LEDS; i++) {
        int value = ring_read(d->fd[i]);
        if (value < 0)
            return 0;
        if (value == d->last[i])
            continue;
        /* The normal clock blinks its colon while our numbers are displayed.
         * Remember the latest colon state, then keep the overlay colon blank. */
        if (i == 28 || i == 29) {
            d->saved[i] = (unsigned char)value;
            if (!ring_write(d->fd[i], d->last[i]))
                return 0;
        } else
            return 0;
    }
    return 1;
}
static void display_show(struct display *d, int percent, int64_t now) {
    static const unsigned char digits[] = {63, 6, 91, 79, 102, 109, 125, 7, 127, 111};
    if (d->active && !display_unchanged(d)) {
        display_close(d);
        return;
    }
    if (!d->active && !display_open(d))
        return;
    if (percent < 0)
        percent = 0;
    if (percent > 100)
        percent = 100;
    unsigned char frame[DISPLAY_LEDS] = {0};
    for (int position = 3; position >= 1; position--) {
        unsigned mask = digits[percent % 10];
        for (int segment = 0; segment < 7; segment++)
            if (mask & (1u << segment))
                frame[position * 7 + segment] = (unsigned char)d->brightness;
        percent /= 10;
        if (!percent)
            break;
    }
    for (int i = 0; i < DISPLAY_LEDS; i++)
        if (frame[i] != d->last[i]) {
            if (!ring_write(d->fd[i], frame[i])) {
                display_close(d);
                return;
            }
            d->last[i] = frame[i];
        }
    d->until = now + 2000000;
    d->check_at = now + 50000;
}
static void display_tick(struct display *d, int64_t now) {
    if (!d->active || now < d->check_at)
        return;
    if (!display_unchanged(d) || now >= d->until) {
        display_close(d);
        return;
    }
    d->check_at = now + 50000;
}
