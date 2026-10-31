package fixtures;

/** Target: the assignment must keep the pre-throw value visible to the catch. */
public final class Case {
    public static int f() {
        int v = 7;
        try {
            v = Dep.alwaysThrows();
        } catch (ExpectedError e) {
            Dep.record(v);
        }
        return v;
    }
}
