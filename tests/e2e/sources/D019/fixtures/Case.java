package fixtures;

/** Target: a null Object-typed argument keeps the Object overload. */
public final class Case {
    public static String target() {
        Object value = null;
        return Dep.pick(value);
    }
}
