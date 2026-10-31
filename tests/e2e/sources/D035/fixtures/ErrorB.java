package fixtures;

/** Self-made outer error type raised from inside the inner handler. */
public final class ErrorB extends RuntimeException {
    public ErrorB() {
        super("B");
    }
}
