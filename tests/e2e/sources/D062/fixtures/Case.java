package fixtures;

/**
 * D062 original source. The class holds only the canonical constructor and
 * public static int f(int x), whose body is the parameter read, one integer
 * increment and the return. There is no branch, payload, handler, call, field
 * or allocation, so the certified raw form is small and complete. Both
 * invocations bind f(I)I, never the constructor.
 */
public class Case {
    public Case() {
    }

    public static int f(int x) {
        return x + 1;
    }
}
