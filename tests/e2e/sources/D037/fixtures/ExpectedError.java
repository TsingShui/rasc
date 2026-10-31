package fixtures;

/** Self-made unchecked error raised inside the monitor scope. */
public final class ExpectedError extends RuntimeException {
    public ExpectedError() {
        super("expected");
    }
}
