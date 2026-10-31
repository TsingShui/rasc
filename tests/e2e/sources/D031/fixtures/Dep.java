package fixtures;

/** Self-made dependency: selects which of the two error types escapes. */
public final class Dep {
    private Dep() {
    }

    public static void raise(int kind) throws BaseError {
        if (kind == 1) {
            throw new SubError();
        }
        if (kind == 2) {
            throw new BaseError();
        }
    }
}
