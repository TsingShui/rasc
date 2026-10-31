package fixtures;

/** Self-made base: its constructor logs BASE, so super must run first. */
public class Base {
    public Base() {
        Dep.log("BASE");
    }
}
