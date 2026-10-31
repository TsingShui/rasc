package fixtures;

/** Self-made unchecked error raised by the failing producer. */
public final class ExpectedError extends RuntimeException {
    public ExpectedError() {
        super("expected");
    }
}
