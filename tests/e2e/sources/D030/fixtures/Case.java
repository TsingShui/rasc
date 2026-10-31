package fixtures;

/** Target: each throw site must expose its own pre-throw value to the shared handler. */
public final class Case {
    public static int f(boolean c) {
        int v = 3;
        try {
            if (c) {
                v = 7;
                Dep.failA();
            } else {
                v = 9;
                Dep.failB();
            }
        } catch (ExpectedError e) {
            Dep.record(v);
        }
        return v;
    }
}
