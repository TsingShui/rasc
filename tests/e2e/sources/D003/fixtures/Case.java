package fixtures;

public final class Case {

    public static int rotate(int a, int b, int c, int n) {
        while (n-- > 0) {
            int t = a;
            a = b;
            b = c;
            c = t;
        }
        return a * 100 + b * 10 + c;
    }
}
