package fixtures;

/** Target: the returned value is the field read before the write. */
public final class Case {
    public static int f(Box x) {
        int old = x.n;
        x.n = old + 3;
        return old;
    }
}
