package fixtures;

public final class Case {

    public static int f(int k) {
        switch (k) {
            case -2147483648:
                return 1;
            case 2147483647:
                return 2;
            default:
                return 3;
        }
    }
}
