package fixtures;

public final class Case {

    public static int count(int n) {
        int s = 0;
        for (int i = 0; i < n; i++) {
            if ((i & 1) == 0) {
                continue;
            }
            s += i;
        }
        return s;
    }
}
