package fixtures;

/** Target: the covariant override keeps its real method and the generated bridge. */
public final class Case extends Base {
    @Override
    public String get() {
        return "child";
    }
}
