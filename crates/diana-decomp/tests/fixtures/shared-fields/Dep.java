package fixtures;
public final class Dep {
    public static Box target;
    public static int calls;
    public static int fail = -1;
    public static String events = "";
    public static int value(int index) {
        calls++;
        events += index + ",";
        if (target != null) target.n += 100;
        if (index == fail) throw new Failure();
        return index + 7;
    }
}
