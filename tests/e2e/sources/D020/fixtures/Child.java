package fixtures;

/** Self-made override: the virtual call must reach this body. */
public class Child extends Base {
    @Override
    public int f() {
        return 2;
    }
}
