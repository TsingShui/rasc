package fixtures;

/** Target: the returned value is read before finally rewrites the local. */
public final class Case {
    public static int f() {
        int v = 3;
        try {
            return v;
        } finally {
            v = 8;
            Dep.log("FINAL:" + v);
        }
    }
}
