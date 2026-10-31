package fixtures;

/** Self-made base: logs B, so a virtual rewrite of super would log C twice. */
public class Base {
    public int f() {
        Dep.log("B");
        return 1;
    }
}
