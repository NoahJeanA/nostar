// Developer tool for TECHNIK.md 9.7 and 14.1, not part of nostar.
// Build: cc -O2 -o hbcmp tools/hbcmp.c $(pkg-config --cflags --libs harfbuzz)
//
//   hbcmp time <original.ttf> <patched.ttf> <text>
//       Shapes every line with both fonts in turn, best of 15 runs each.
//       Prints: <original ms> <patched ms> <text bytes>
//   hbcmp shape <font.ttf> <text>
//       Prints glyph/advance pairs, one line of output per line of text.
#include <hb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e3 + t.tv_nsec / 1e6;
}

static hb_font_t *load(const char *path) {
    hb_blob_t *b = hb_blob_create_from_file_or_fail(path);
    if (!b) {
        fprintf(stderr, "hbcmp: read %s\n", path);
        exit(2);
    }
    return hb_font_create(hb_face_create(b, 0));
}

static char **read_lines(const char *path, size_t *n, size_t *bytes) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "hbcmp: read %s\n", path);
        exit(2);
    }
    char **lines = NULL, *line = NULL;
    size_t cap = 0, lcap = 0;
    ssize_t len;
    *n = 0;
    *bytes = 0;
    while ((len = getline(&line, &lcap, f)) > 0) {
        *bytes += len;
        if (line[len - 1] == '\n') line[--len] = 0;
        if (*n == cap) {
            cap = cap ? cap * 2 : 1024;
            lines = realloc(lines, cap * sizeof *lines);
        }
        lines[(*n)++] = strdup(line);
    }
    fclose(f);
    return lines;
}

static void shape(hb_font_t *font, hb_buffer_t *buf, const char *text) {
    hb_buffer_reset(buf);
    hb_buffer_add_utf8(buf, text, -1, 0, -1);
    hb_buffer_guess_segment_properties(buf);
    hb_shape(font, buf, NULL, 0);
}

static double run(hb_font_t *font, hb_buffer_t *buf, char **lines, size_t n) {
    double t = now_ms();
    for (size_t i = 0; i < n; i++) shape(font, buf, lines[i]);
    return now_ms() - t;
}

int main(int argc, char **argv) {
    hb_buffer_t *buf = hb_buffer_create();
    size_t n, bytes;
    if (argc == 5 && strcmp(argv[1], "time") == 0) {
        hb_font_t *a = load(argv[2]), *b = load(argv[3]);
        char **lines = read_lines(argv[4], &n, &bytes);
        double ba = 1e18, bb = 1e18;
        for (int r = 0; r < 15; r++) {
            double ta = run(a, buf, lines, n), tb = run(b, buf, lines, n);
            if (ta < ba) ba = ta;
            if (tb < bb) bb = tb;
        }
        printf("%.2f %.2f %zu\n", ba, bb, bytes);
        return 0;
    }
    if (argc == 4 && strcmp(argv[1], "shape") == 0) {
        hb_font_t *font = load(argv[2]);
        char **lines = read_lines(argv[3], &n, &bytes);
        for (size_t i = 0; i < n; i++) {
            shape(font, buf, lines[i]);
            unsigned count;
            hb_glyph_info_t *info = hb_buffer_get_glyph_infos(buf, &count);
            hb_glyph_position_t *pos = hb_buffer_get_glyph_positions(buf, &count);
            for (unsigned j = 0; j < count; j++) printf("%u/%d ", info[j].codepoint, pos[j].x_advance);
            printf("\n");
        }
        return 0;
    }
    fprintf(stderr, "usage: hbcmp time <original.ttf> <patched.ttf> <text>\n"
                    "       hbcmp shape <font.ttf> <text>\n");
    return 2;
}
