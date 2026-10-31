package fixtures;

public final class Case {

    public static int f(int n) {
        int i = 0;
        int s = 0;
        while (i < n) {
            if (i == 3) {
                break;
            }
            s += i;
            i++;
        }
        return s * 10 + i;
    }
}
