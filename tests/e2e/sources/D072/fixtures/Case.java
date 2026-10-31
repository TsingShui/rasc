package fixtures;

public final class Case {

    public static int sum(byte a, byte b) {
        return a + b;
    }

    public static int mix(boolean p, byte b, char c) {
        int v;
        if (p) {
            v = b;
        } else {
            v = c;
        }
        return v;
    }
}
