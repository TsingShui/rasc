package fixtures;

/** Target: nested scopes on one lock must reenter and unwind exactly once each. */
public final class Case {
    public static boolean f(Object lock) {
        synchronized (lock) {
            synchronized (lock) {
                Dep.log("IN");
            }
            return Thread.holdsLock(lock);
        }
    }
}
