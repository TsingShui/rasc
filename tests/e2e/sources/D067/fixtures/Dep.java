package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made callee: two distinct call sites in one method must stay distinct. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();

    private Dep() {
    }

    public static void reset() {
        events.clear();
    }

    public static void f() {
        events.add("F");
    }
}
