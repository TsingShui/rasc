package fixtures;

/** Target: the cleanup sits outside the try, so the old handler must not see it. */
public final class Case {
    public static int f() {
        int r;
        try {
            r = Dep.ok();
        } catch (ExpectedError e) {
            r = -1;
        }
        Dep.cleanupThrows();
        return r;
    }
}
