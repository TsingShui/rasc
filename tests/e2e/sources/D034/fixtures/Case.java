package fixtures;

/** Target: a throwing finally discards the pending return of 3. */
public final class Case {
    public static int f() {
        try {
            return 3;
        } finally {
            Dep.finalThrows();
        }
    }
}
