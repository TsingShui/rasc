package fixtures;

public final class Case {

    public static int f(int k) {
        int r = 0;
        switch (k) {
            case -3:
            case 1:
                r = 10;
            case 2:
                r += 2;
                break;
            default:
                r = 7;
        }
        return r;
    }
}
