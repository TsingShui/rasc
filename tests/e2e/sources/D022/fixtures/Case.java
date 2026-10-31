package fixtures;

/** Target: the unused result must not let dead-code elimination drop the call. */
public final class Case {
    public static void target() {
        Dep.bumpAndThrow();
    }
}
