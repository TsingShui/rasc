package fixtures;

/** Self-made checked subtype: it must win over the wider handler that follows. */
public final class SubError extends BaseError {
    public SubError() {
        super();
    }
}
