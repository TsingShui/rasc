package fixtures;

public class Conditions {
    public static int classNull() { return Conditions.class == null ? 0 : 1; }
    public static int classNonNull() { return Conditions.class != null ? 1 : 0; }
    public static int sameClass() { return Conditions.class == Conditions.class ? 1 : 0; }
}
