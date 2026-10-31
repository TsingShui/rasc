package fixtures;

public final class Case {

    public static int f(int a, int n) {
        return a >>> n;
    }

    public static long g(long a, int n) {
        return a << n;
    }
}
