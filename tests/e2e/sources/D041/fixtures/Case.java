package fixtures;

/** Target: the implicit super() call must precede the constructor body effect. */
public final class Case extends Base {
    public Case() {
        super();
        Dep.log("BODY");
    }
}
