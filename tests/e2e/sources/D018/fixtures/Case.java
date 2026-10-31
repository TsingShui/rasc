package fixtures;

/** Target: the receiver is evaluated first, then the arguments left to right. */
public final class Case {
    public static int target() {
        return Dep.receiver().sum(Dep.arg(1), Dep.arg(2));
    }
}
