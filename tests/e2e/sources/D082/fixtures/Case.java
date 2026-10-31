package fixtures;

/** Target: a parameter lock inside a bounded loop must stay a recovered scope. */
public final class Case {
    public static int f(Object lock, int n) {
        int s = 0;
        for (int i = 0; i < n; i++) {
            synchronized (lock) {
                if (Thread.holdsLock(lock)) {
                    s++;
                }
            }
        }
        return s;
    }
}
