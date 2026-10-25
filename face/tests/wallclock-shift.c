/* wallclock-shift: an LD_PRELOAD shim for the face's clock checks only.
 * It adds an offset to the wall clock (CLOCK_REALTIME, gettimeofday, time)
 * of one process and leaves CLOCK_MONOTONIC alone. The offset, in
 * milliseconds, is read from the file named by WALLCLOCK_SHIFT_FILE, at most
 * every 50 ms, so a check can step the wall clock of a running face.
 *
 * libfaketime cannot do this here: Quickshell links jemalloc, and
 * libfaketime's dlsym during jemalloc's start-up deadlocks (checked with
 * libfaketime 0.9.11; even `quickshell --version` hangs). This shim calls the
 * kernel directly and never allocates, so it needs no dlsym.
 *
 * Built by tests/clock-checks.sh: cc -shared -fPIC -O2 -o shift.so wallclock-shift.c
 */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

static long long shift_ms;
static long long checked_ns;

static long long mono_ns(void) {
    struct timespec m;
    syscall(SYS_clock_gettime, CLOCK_MONOTONIC, &m);
    return m.tv_sec * 1000000000LL + m.tv_nsec;
}

static long long shift_now_ns(void) {
    long long now = mono_ns();
    long long last = __atomic_load_n(&checked_ns, __ATOMIC_RELAXED);
    if (last == 0 || now - last > 50000000LL) {
        __atomic_store_n(&checked_ns, now, __ATOMIC_RELAXED);
        const char *path = getenv("WALLCLOCK_SHIFT_FILE");
        if (path) {
            int fd = (int)syscall(SYS_openat, AT_FDCWD, path, O_RDONLY | O_CLOEXEC);
            if (fd >= 0) {
                char buf[32];
                long n = syscall(SYS_read, fd, buf, sizeof buf - 1);
                syscall(SYS_close, fd);
                if (n > 0) {
                    buf[n] = 0;
                    long long v = 0, sign = 1;
                    const char *p = buf;
                    if (*p == '-') { sign = -1; p++; } else if (*p == '+') p++;
                    while (*p >= '0' && *p <= '9') v = v * 10 + (*p++ - '0');
                    __atomic_store_n(&shift_ms, sign * v, __ATOMIC_RELAXED);
                }
            }
        }
    }
    return __atomic_load_n(&shift_ms, __ATOMIC_RELAXED) * 1000000LL;
}

int clock_gettime(clockid_t clk, struct timespec *ts) {
    long r = syscall(SYS_clock_gettime, clk, ts);
    if (r == 0 && (clk == CLOCK_REALTIME || clk == CLOCK_REALTIME_COARSE)) {
        long long ns = ts->tv_sec * 1000000000LL + ts->tv_nsec + shift_now_ns();
        ts->tv_sec = ns / 1000000000LL;
        ts->tv_nsec = ns % 1000000000LL;
    }
    return (int)r;
}

int gettimeofday(struct timeval *tv, void *tz) {
    (void)tz;
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    if (tv) { tv->tv_sec = ts.tv_sec; tv->tv_usec = ts.tv_nsec / 1000; }
    return 0;
}

time_t time(time_t *t) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    if (t) *t = ts.tv_sec;
    return ts.tv_sec;
}
