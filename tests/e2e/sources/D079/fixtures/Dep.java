package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency shared by both recovered classes. */
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
