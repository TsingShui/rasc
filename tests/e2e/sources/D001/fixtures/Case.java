package fixtures;

public final class Case {

    public static int choose(boolean c, int a, int b) {
        int v;
        if (c) {
            v = a;
        } else {
            v = b;
        }
        return v;
    }
}
