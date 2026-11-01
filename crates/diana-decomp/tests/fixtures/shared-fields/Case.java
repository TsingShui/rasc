package fixtures;
public final class Case {
    public static int combine(Box b, int[] a, int index) {
        int old = b.n;
        int next = Dep.value(index);
        a[index] = old;
        b.n = next;
        return b.n + a[index];
    }
    public static int repeat(Box b, int[] a, int count) {
        for (int i = 0; i < count; i++) {
            int old = b.n;
            int next = Dep.value(i);
            a[i] = old;
            b.n = next;
        }
        return b.n;
    }
    public static void unused(Box b, int[] a) { int old = b.n; int length = a.length; }
}
