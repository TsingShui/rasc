package fixtures;

public final class Case {

    public static int nested() {
        int s = 0;
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 4; j++) {
                if (i == 1 && j == 2) {
                    break outer;
                }
                s++;
            }
        }
        return s;
    }
}
