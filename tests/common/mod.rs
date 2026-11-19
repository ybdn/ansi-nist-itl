//! Complexity checks shared by the test files.

/// CPU time of the calling thread, in seconds: unlike the wall clock, it
/// does not count the time other threads or processes take the core.
#[cfg(unix)]
fn thread_cpu_time() -> f64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid `timespec` the call writes to.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID)");
    t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9
}

/// Elsewhere, the wall clock (not tested on a loaded machine).
#[cfg(not(unix))]
fn thread_cpu_time() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// CPU time of one run of `f`, in seconds.
fn cpu_time(f: impl Fn()) -> f64 {
    let start = thread_cpu_time();
    f();
    thread_cpu_time() - start
}

/// Fails if `run` on `build(16 n)` takes more than 64 times the CPU time it
/// takes on `build(n)`: about 16 when linear, 256 when quadratic, a margin
/// of 4 on either side. `n` is chosen so that the small case takes some
/// milliseconds in a debug build. The small case is the least of five
/// runs; the large one passes at its first run under the bound, and fails
/// at once above twice the bound, so that quadratic code fails quickly.
pub fn assert_linear<T>(what: &str, n: usize, build: impl Fn(usize) -> T, run: impl Fn(&T)) {
    let (small, large) = (build(n), build(16 * n));
    let t_small = (0..5)
        .map(|_| cpu_time(|| run(&small)))
        .fold(f64::MAX, f64::min);
    let mut ratio = f64::MAX;
    for _ in 0..3 {
        ratio = ratio.min(cpu_time(|| run(&large)) / t_small);
        if ratio < 64.0 || ratio > 128.0 {
            break;
        }
    }
    assert!(
        ratio < 64.0,
        "{what}: 16 times the input takes {ratio:.1} times the CPU time of {:.1} ms",
        t_small * 1e3
    );
}
