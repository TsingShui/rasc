package fixtures;
public final class Case {
    public static Box pick(Box a, Box b, int flag) { if (flag == 0) return a; return b; }
    public static int tags(Box b, Token t, int flag) {
        if (flag == 0) return b == null ? 1 : 2;
        return t == null ? 3 : 4;
    }
    public static int storeBoth(Object[] a, Box b, Token t) {
        a[0] = b; a[1] = t; return a.length;
    }
    public static int repeat(Object[] a, Box b, Token t, int count) {
        for (int i = 0; i < count; i++) { a[i] = b; a[i + 1] = t; }
        return a.length;
    }
}
