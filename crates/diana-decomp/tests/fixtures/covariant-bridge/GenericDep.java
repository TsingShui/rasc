package fixtures;

public class GenericDep {
    public static <T> T identity(T value) { return value; }
}
