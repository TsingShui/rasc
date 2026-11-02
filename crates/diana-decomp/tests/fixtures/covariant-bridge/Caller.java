package fixtures;

public class Caller {
    public static Object call(Object value) { return GenericDep.identity(value); }
}
