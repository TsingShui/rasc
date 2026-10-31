package fixtures;

/** Self-made unchecked error raised by both failing producers. */
public final class ExpectedError extends RuntimeException {
    public ExpectedError() {
        super("expected");
    }
}
