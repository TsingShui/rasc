package fixtures;

/** Target: the bounds and null checks must stay after the index effect. */
public final class Case {
    public static int f(int[] a) {
        int i = Dep.index();
        return a[i];
    }
}
