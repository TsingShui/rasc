package fixtures;

/** Target: a blank final field must be assigned exactly once in each branch. */
public final class Case {
    public final int x;

    public Case(boolean c) {
        if (c) {
            x = 1;
        } else {
            x = 2;
        }
    }
}
