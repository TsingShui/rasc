package fixtures;
public final class ThrowDep {
    public static String trace = "";
    public static int choose(boolean fail) {
        trace += "C";
        if (fail) throw new ThrowError();
        return 11;
    }
}
