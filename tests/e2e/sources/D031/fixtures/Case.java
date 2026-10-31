package fixtures;

/** Target: handler precedence follows the declared catch order, not type width. */
public final class Case {
    public static int f(int k) {
        try {
            Dep.raise(k);
            return 0;
        } catch (SubError e) {
            return 1;
        } catch (BaseError e) {
            return 2;
        }
    }
}
