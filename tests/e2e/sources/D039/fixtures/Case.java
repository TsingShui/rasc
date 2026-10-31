package fixtures;

/** Target: the lock expression is evaluated once, not again at exit. */
public final class Case {
    public static void f() {
        synchronized (Dep.nextLock()) {
            Dep.log("BODY");
        }
    }
}
