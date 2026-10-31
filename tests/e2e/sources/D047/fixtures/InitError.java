package fixtures;

/** Self-made unchecked error raised from the static initializer. */
public final class InitError extends RuntimeException {
    public InitError() {
        super("init");
    }
}
