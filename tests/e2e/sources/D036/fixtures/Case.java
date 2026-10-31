package fixtures;

/** Target: the body call must run while the monitor is held. */
public final class Case {
    public static boolean f(Object lock) {
        synchronized (lock) {
            return Thread.holdsLock(lock);
        }
    }
}
