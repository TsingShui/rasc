package fixtures;

/** Self-made inner error type. */
public final class ErrorA extends RuntimeException {
    public ErrorA() {
        super("A");
    }
}
