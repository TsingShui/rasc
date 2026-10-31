package fixtures;

/** Self-made error raised from the cleanup path. */
public final class FinalError extends RuntimeException {
    public FinalError() {
        super("final");
    }
}
