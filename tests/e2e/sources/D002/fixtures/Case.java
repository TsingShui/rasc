package fixtures;

public final class Case {

    public static int swap(int a, int b, int n) {
        for (int i = 0; i < n; i++) {
            int t = a;
            a = b;
            b = t;
        }
        return a * 100 + b;
    }
}
