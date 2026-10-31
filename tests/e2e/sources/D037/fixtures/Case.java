package fixtures;

/** Target: the release handler must run on the abnormal exit path. */
public final class Case {
    public static void f(Object lock) {
        synchronized (lock) {
            Dep.raise();
        }
    }
}
