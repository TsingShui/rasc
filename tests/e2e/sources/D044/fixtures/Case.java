package fixtures;

/** Target: the static field trigger must run the initializer once, not per call. */
public final class Case {
    public static int n = Dep.init();

    public static int get() {
        return n;
    }
}
