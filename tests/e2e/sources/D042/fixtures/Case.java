package fixtures;

/** Target: the this(...) delegation must not repeat the delegated initialization. */
public final class Case {
    public int n;

    public Case() {
        this(4);
        Dep.log("ZERO");
    }

    public Case(int n) {
        this.n = n;
        Dep.log("INT:" + n);
    }
}
