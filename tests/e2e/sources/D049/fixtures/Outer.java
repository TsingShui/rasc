package fixtures;

/**
 * Target owner: the inner class captures this outer instance. The field is
 * package-private so no unknown auto-generated accessor is needed.
 */
public class Outer {
    int n = 7;

    /** Inner capture: reads the captured outer instance's field. */
    public class Inner {
        public int f() {
            return Outer.this.n;
        }
    }
}
