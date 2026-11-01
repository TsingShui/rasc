package fixtures;
public final class Dep {
    public static String trace = "";
    public static int choose(boolean fail) throws Checked {
        trace += "C";
        if (fail) throw new Checked();
        return 11;
    }
    public static void cleanup(boolean fail) {
        trace += "F";
        if (fail) throw new IllegalArgumentException();
    }
}
