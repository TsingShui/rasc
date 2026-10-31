package fixtures;

/** Self-made holder: the constructor logs the argument it stores. */
public final class Box {
    public final int n;

    public Box(int n) {
        this.n = n;
        Dep.log("CTOR:" + n);
    }
}
