package fixtures;

/** Target: the override must reach the base body through invoke-super. */
public class Child extends Base {
    @Override
    public int f() {
        Dep.log("C");
        return super.f() + 1;
    }
}
