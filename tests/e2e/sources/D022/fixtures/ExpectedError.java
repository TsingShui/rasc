package fixtures;

/** Self-made unchecked error raised by the effectful producer. */
public final class ExpectedError extends RuntimeException {
    public ExpectedError() {
        super("expected");
    }
}
