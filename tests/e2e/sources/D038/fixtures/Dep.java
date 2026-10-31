package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: logs the inner body so the nesting is observable. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();

    private Dep() {
    }

    public static void reset() {
        events.clear();
    }

    public static void log(String label) {
        events.add(label);
    }
}
