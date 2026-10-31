package fixtures;

/** Self-made unchecked error raised by both the producer and the cleanup. */
public final class ExpectedError extends RuntimeException {
    public ExpectedError() {
        super("expected");
    }
}
