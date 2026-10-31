package fixtures;

/** Target: the array store keeps the runtime component-type check of String[]. */
public final class Case {
    public static void f(Object[] a, Object v) {
        a[0] = v;
    }
}
