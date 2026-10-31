package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made overloaded dependency: the descriptor selects the overload. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();

    private Dep() {
    }

    public static void reset() {
        events.clear();
    }

    public static String pick(Object value) {
        events.add("pick:Object");
        return "O";
    }

    public static String pick(String value) {
        events.add("pick:String");
        return "S";
    }
}
